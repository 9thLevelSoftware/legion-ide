#![expect(clippy::unwrap_used)] // TODO(emilk): remove unwraps

use std::sync::Arc;

use emath::{Align, GuiRounding as _, NumExt as _, Pos2, Rect, Vec2, pos2, vec2};

use crate::{
    Color32, Mesh, Stroke, Vertex,
    stroke::PathStroke,
    text::{
        font::{FontFace, StyledMetrics, is_cjk, is_cjk_break_allowed},
        fonts::FontFaceKey,
    },
};

use super::{
    FontsImpl, Galley, Glyph, LayoutJob, LayoutSection, PlacedRow, Row, RowVisuals, TextFormat,
};

// ----------------------------------------------------------------------------

/// Represents GUI scale and convenience methods for rounding to pixels.
#[derive(Clone, Copy)]
struct PointScale {
    pub pixels_per_point: f32,
}

impl PointScale {
    #[inline(always)]
    pub fn new(pixels_per_point: f32) -> Self {
        Self { pixels_per_point }
    }

    #[inline(always)]
    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    #[inline(always)]
    pub fn round_to_pixel(&self, point: f32) -> f32 {
        (point * self.pixels_per_point).round() / self.pixels_per_point
    }

    #[inline(always)]
    pub fn floor_to_pixel(&self, point: f32) -> f32 {
        (point * self.pixels_per_point).floor() / self.pixels_per_point
    }
}

// ----------------------------------------------------------------------------

/// The glyph-free state needed to continue shaping at an exact source offset.
///
/// This is deliberately separate from [`Paragraph`], whose glyph vector is
/// retained by the ordinary layout path only. A streaming continuation can
/// therefore be cloned without copying any output-sized storage.
#[derive(Clone, Copy, Debug, PartialEq)]
struct ShapeState {
    /// Start of the next glyph to be added. In screen-space / physical pixels.
    cursor_x_px: f32,

    /// Previous glyph identity used by pair kerning.
    last_glyph_id: Option<skrifa::GlyphId>,
}

impl Default for ShapeState {
    fn default() -> Self {
        Self {
            cursor_x_px: 0.0,
            last_glyph_id: None,
        }
    }
}

impl ShapeState {
    #[inline]
    fn apply_kerning(
        &mut self,
        font_face: Option<&crate::text::font::FontFace>,
        font_face_metrics: &StyledMetrics,
        glyph_id: Option<skrifa::GlyphId>,
        extra_letter_spacing: f32,
        pixels_per_point: f32,
    ) {
        if let (Some(font_face), Some(last_glyph_id), Some(glyph_id)) =
            (font_face, self.last_glyph_id, glyph_id)
        {
            self.cursor_x_px +=
                font_face.pair_kerning_pixels(font_face_metrics, last_glyph_id, glyph_id);
            self.cursor_x_px += extra_letter_spacing * pixels_per_point;
        }
    }

    #[inline]
    fn commit_glyph(&mut self, advance_width_px: f32, glyph_id: skrifa::GlyphId) {
        self.cursor_x_px += advance_width_px;
        self.last_glyph_id = Some(glyph_id);
    }
}

/// Temporary storage before line-wrapping.
#[derive(Clone)]
struct Paragraph {
    pub shape: ShapeState,

    /// This is included in case there are no glyphs
    pub section_index_at_start: u32,

    pub glyphs: Vec<Glyph>,

    /// In case of an empty paragraph ("\n"), use this as height.
    pub empty_paragraph_height: f32,
}

impl Paragraph {
    pub fn from_section_index(section_index_at_start: u32) -> Self {
        Self {
            shape: ShapeState::default(),
            section_index_at_start,
            glyphs: vec![],
            empty_paragraph_height: 0.0,
        }
    }
}

/// Opaque state for the bounded, unwrapped single-format layout path.
///
/// The fields intentionally remain private so callers cannot reset pen or
/// kerning state and accidentally change rendered geometry.
#[derive(Clone)]
pub struct UnwrappedLayoutContinuation {
    source_key: u128,
    expected_byte: u64,
    format: TextFormat,
    pixels_per_point: f32,
    font_identity: Arc<()>,
    shape: ShapeState,
    initial_byte: u64,
}

/// Exact result of a fully consumed unwrapped pass.
///
/// The proof fields are private so a caller cannot manufacture a summary for
/// a different source, font set, format, or scale. The summary contains no
/// text or glyph storage.
#[derive(Clone)]
pub struct UnwrappedLayoutSummary {
    source_key: u128,
    initial_byte: u64,
    final_byte: u64,
    precise_advance_px: f32,
    format: TextFormat,
    pixels_per_point: f32,
    font_identity: Arc<()>,
}

impl UnwrappedLayoutSummary {
    /// Absolute byte range covered by the completed pass.
    pub fn byte_span(&self) -> std::ops::Range<u64> {
        self.initial_byte..self.final_byte
    }

    /// Absolute starting byte of the completed pass.
    pub fn start_byte(&self) -> u64 {
        self.initial_byte
    }

    /// Absolute byte immediately after the completed pass.
    pub fn end_byte(&self) -> u64 {
        self.final_byte
    }

    /// The exact final pen advance in logical points.
    pub fn precise_width(&self) -> f32 {
        self.precise_advance_px / self.pixels_per_point
    }

    /// Validate that this summary can be consumed by a later pass over the
    /// same immutable source and layout identity.
    pub(crate) fn validate_for_layout(
        &self,
        source_key: u128,
        expected_span: std::ops::Range<u64>,
        format: &TextFormat,
        pixels_per_point: f32,
        font_identity: &Arc<()>,
    ) -> Result<(), UnwrappedLayoutError> {
        if self.source_key != source_key
            || self.byte_span() != expected_span
            || self.format != *format
            || self.pixels_per_point != pixels_per_point
            || !Arc::ptr_eq(&self.font_identity, font_identity)
        {
            return Err(UnwrappedLayoutError::ChangedLayoutKey);
        }
        Ok(())
    }
}

impl std::fmt::Debug for UnwrappedLayoutSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnwrappedLayoutSummary")
            .field("byte_span", &self.byte_span())
            .field("precise_width", &self.precise_width())
            .finish_non_exhaustive()
    }
}

pub struct UnwrappedGlyphBatch {
    pub glyphs: Vec<Glyph>,
    pub consumed_bytes: usize,
    pub continuation: Option<UnwrappedLayoutContinuation>,
    pub status: UnwrappedLayoutStatus,
    pub summary: Option<UnwrappedLayoutSummary>,
}

impl std::fmt::Debug for UnwrappedLayoutContinuation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnwrappedLayoutContinuation")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for UnwrappedGlyphBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("UnwrappedGlyphBatch")
            .field("glyph_count", &self.glyphs.len())
            .field("consumed_bytes", &self.consumed_bytes)
            .field("has_continuation", &self.continuation.is_some())
            .field("status", &self.status)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnwrappedLayoutStatus {
    NeedMoreInput,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnwrappedLayoutError {
    InvalidFormat,
    ChangedLayoutKey,
    InputChunkTooLarge,
    OffsetOverflow,
    OutputBudgetOutOfRange,
    NoProgress,
}

/// A glyph's atlas-independent geometry for streaming text measurement.
///
/// This deliberately has no UV or paintable allocation. The byte range is
/// absolute in the source passed to [`layout_unwrapped_metrics_chunk`].
#[derive(Clone, Debug, PartialEq)]
pub struct MetricGlyph {
    pub chr: char,
    pub source_byte_range: std::ops::Range<u64>,
    pub physical_x: i32,
    pub logical_x: f32,
    pub advance_width: f32,
    pub line_height: f32,
    pub font_face_height: f32,
    pub font_face_ascent: f32,
    pub font_height: f32,
    pub font_ascent: f32,
}

impl MetricGlyph {
    pub fn byte_range(&self) -> std::ops::Range<u64> {
        self.source_byte_range.clone()
    }

    pub fn snapped_x(&self) -> f32 {
        self.physical_x as f32
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricLayoutStatus {
    NeedMoreInput,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MetricLayoutError {
    InvalidFormat,
    ChangedLayoutKey,
    InputChunkTooLarge,
    OffsetOverflow,
    OutputBudgetOutOfRange,
    NoProgress,
}

/// Opaque glyph-free checkpoint for the atlas-independent metric scan.
#[derive(Clone)]
pub struct MetricLayoutContinuation {
    source_key: u128,
    expected_byte: u64,
    format: TextFormat,
    pixels_per_point: f32,
    metric_identity: Arc<()>,
    shape: ShapeState,
    initial_byte: u64,
}

#[derive(Clone)]
pub struct MetricLayoutSummary {
    source_key: u128,
    initial_byte: u64,
    final_byte: u64,
    precise_advance_px: f32,
    format: TextFormat,
    pixels_per_point: f32,
    metric_identity: Arc<()>,
}

impl MetricLayoutSummary {
    pub fn byte_span(&self) -> std::ops::Range<u64> {
        self.initial_byte..self.final_byte
    }

    pub fn start_byte(&self) -> u64 {
        self.initial_byte
    }

    pub fn end_byte(&self) -> u64 {
        self.final_byte
    }

    pub fn precise_width(&self) -> f32 {
        self.precise_advance_px / self.pixels_per_point
    }

    pub(crate) fn validate_for_layout(
        &self,
        source_key: u128,
        expected_span: std::ops::Range<u64>,
        format: &TextFormat,
        pixels_per_point: f32,
        metric_identity: &Arc<()>,
    ) -> Result<(), MetricLayoutError> {
        if self.source_key != source_key
            || self.byte_span() != expected_span
            || self.format != *format
            || self.pixels_per_point != pixels_per_point
            || !Arc::ptr_eq(&self.metric_identity, metric_identity)
        {
            return Err(MetricLayoutError::ChangedLayoutKey);
        }
        Ok(())
    }
}

pub struct MetricGlyphBatch {
    pub glyphs: Vec<MetricGlyph>,
    pub consumed_bytes: usize,
    pub continuation: Option<MetricLayoutContinuation>,
    pub status: MetricLayoutStatus,
    pub summary: Option<MetricLayoutSummary>,
}

#[derive(Clone, Debug)]
struct WrappedRowCandidate {
    end_byte: u64,
    post: ShapeState,
    next_x: f32,
    last_x: f32,
    advance: f32,
    following_max_x: f32,
}

#[derive(Clone)]
pub struct WrappedRowContinuation {
    source_key: u128,
    expected_byte: u64,
    format: TextFormat,
    pixels_per_point: f32,
    wrap_width: f32,
    break_anywhere: bool,
    fit_to_summary: bool,
    metric_identity: Arc<()>,
    shape: ShapeState,
    row_start_shape: ShapeState,
    row_start_byte: u64,
    row_start_x: f32,
    first_row_indentation: f32,
    candidates: [Option<WrappedRowCandidate>; 6],
    pending: Option<(MetricGlyph, ShapeState)>,
    row_first_x: Option<f32>,
    row_max_x: f32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WrappedRowDescriptor {
    source_byte_range: std::ops::Range<u64>,
    row_start_x: f32,
    width: f32,
    line_height: f32,
    row_start_shape: ShapeState,
    source_key: u128,
    format: TextFormat,
    pixels_per_point: f32,
    metric_identity: Arc<()>,
}

impl WrappedRowDescriptor {
    pub fn source_byte_range(&self) -> std::ops::Range<u64> {
        self.source_byte_range.clone()
    }

    pub fn row_start_x(&self) -> f32 {
        self.row_start_x
    }

    pub fn width(&self) -> f32 {
        self.width
    }

    pub fn line_height(&self) -> f32 {
        self.line_height
    }
}

pub struct WrappedRowBatch {
    pub rows: Vec<WrappedRowDescriptor>,
    pub consumed_bytes: usize,
    pub continuation: Option<WrappedRowContinuation>,
    pub status: WrappedRowStatus,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrappedRowStatus {
    NeedMoreInput,
    Complete,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WrappedRowError {
    InvalidFormat,
    ChangedLayoutKey,
    InputChunkTooLarge,
    OffsetOverflow,
    OutputBudgetOutOfRange,
    NoProgress,
}

pub(crate) fn replay_wrapped_row_chunk(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    descriptor: &WrappedRowDescriptor,
    chunk_start_byte: u64,
    chunk: &str,
    is_final_chunk: bool,
    continuation: Option<UnwrappedLayoutContinuation>,
    max_output_glyphs: usize,
) -> Result<UnwrappedGlyphBatch, UnwrappedLayoutError> {
    let identity = fonts.layout_identity();
    let metric_identity = fonts.metric_identity();
    let Some(chunk_end) = chunk_start_byte.checked_add(chunk.len() as u64) else {
        return Err(UnwrappedLayoutError::OffsetOverflow);
    };
    let valid_range = chunk_start_byte >= descriptor.source_byte_range.start
        && chunk_end <= descriptor.source_byte_range.end
        && (!is_final_chunk || chunk_end == descriptor.source_byte_range.end)
        && (continuation.is_some() || chunk_start_byte == descriptor.source_byte_range.start);
    if pixels_per_point != descriptor.pixels_per_point
        || !valid_range
        || !Arc::ptr_eq(&metric_identity, &descriptor.metric_identity)
    {
        return Err(UnwrappedLayoutError::ChangedLayoutKey);
    }
    let continuation = continuation.or_else(|| {
        Some(UnwrappedLayoutContinuation {
            source_key: descriptor.source_key,
            expected_byte: descriptor.source_byte_range.start,
            format: descriptor.format.clone(),
            pixels_per_point,
            font_identity: identity.clone(),
            shape: descriptor.row_start_shape,
            initial_byte: descriptor.source_byte_range.start,
        })
    });
    let mut batch = layout_unwrapped_chunk(
        fonts,
        pixels_per_point,
        identity,
        descriptor.format.clone(),
        descriptor.source_key,
        chunk_start_byte,
        chunk,
        is_final_chunk,
        continuation,
        max_output_glyphs,
    )?;
    for glyph in &mut batch.glyphs {
        glyph.pos.x -= descriptor.row_start_x;
    }
    Ok(batch)
}

impl std::fmt::Debug for WrappedRowContinuation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WrappedRowContinuation")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for MetricLayoutContinuation {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MetricLayoutContinuation")
            .finish_non_exhaustive()
    }
}

impl std::fmt::Debug for MetricGlyphBatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("MetricGlyphBatch")
            .field("glyph_count", &self.glyphs.len())
            .field("consumed_bytes", &self.consumed_bytes)
            .field("has_continuation", &self.continuation.is_some())
            .field("status", &self.status)
            .finish()
    }
}

/// Scan one bounded, newline-free, single-format chunk without touching the
/// atlas. Pen and kerning state is retained in a constant-sized checkpoint.
pub(crate) fn layout_unwrapped_metrics_chunk(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    metric_identity: Arc<()>,
    format: TextFormat,
    source_key: u128,
    chunk_start_byte: u64,
    chunk: &str,
    is_final_chunk: bool,
    continuation: Option<MetricLayoutContinuation>,
    max_output_glyphs: usize,
) -> Result<MetricGlyphBatch, MetricLayoutError> {
    const MAX_CHUNK_BYTES: usize = 96 * 1024;
    if chunk.len() > MAX_CHUNK_BYTES {
        return Err(MetricLayoutError::InputChunkTooLarge);
    }
    let chunk_end = chunk_start_byte
        .checked_add(chunk.len() as u64)
        .ok_or(MetricLayoutError::OffsetOverflow)?;
    if !(1..=4096).contains(&max_output_glyphs) {
        return Err(MetricLayoutError::OutputBudgetOutOfRange);
    }
    if chunk.contains('\n')
        || !pixels_per_point.is_finite()
        || pixels_per_point <= 0.0
        || !format.font_id.size.is_finite()
        || format.font_id.size <= 0.0
        || !format.extra_letter_spacing.is_finite()
    {
        return Err(MetricLayoutError::InvalidFormat);
    }

    let mut state = if let Some(state) = continuation {
        if state.source_key != source_key
            || state.expected_byte != chunk_start_byte
            || state.format != format
            || state.pixels_per_point != pixels_per_point
            || !Arc::ptr_eq(&state.metric_identity, &metric_identity)
        {
            return Err(MetricLayoutError::ChangedLayoutKey);
        }
        state
    } else {
        MetricLayoutContinuation {
            source_key,
            expected_byte: chunk_start_byte,
            format: format.clone(),
            pixels_per_point,
            metric_identity,
            shape: ShapeState::default(),
            initial_byte: chunk_start_byte,
        }
    };

    let line_height = format.line_height.unwrap_or_else(|| {
        fonts
            .font(&format.font_id.family)
            .styled_metrics(pixels_per_point, format.font_id.size, &format.coords)
            .row_height
    });
    let mut glyphs = Vec::with_capacity(max_output_glyphs.min(chunk.len()));
    let consumed = shape_metric_chars(
        &mut fonts.font(&format.font_id.family),
        pixels_per_point,
        &format,
        chunk_start_byte,
        chunk,
        line_height,
        &mut state.shape,
        &mut glyphs,
        max_output_glyphs,
    );
    if !chunk.is_empty() && consumed == 0 {
        return Err(MetricLayoutError::NoProgress);
    }
    state.expected_byte = chunk_start_byte
        .checked_add(consumed as u64)
        .ok_or(MetricLayoutError::OffsetOverflow)?;
    debug_assert!(state.expected_byte <= chunk_end);
    let complete = is_final_chunk && consumed == chunk.len();
    let summary = complete.then(|| MetricLayoutSummary {
        source_key: state.source_key,
        initial_byte: state.initial_byte,
        final_byte: state.expected_byte,
        precise_advance_px: state.shape.cursor_x_px,
        format: state.format.clone(),
        pixels_per_point: state.pixels_per_point,
        metric_identity: Arc::clone(&state.metric_identity),
    });
    if let Some(summary) = &summary {
        summary.validate_for_layout(
            state.source_key,
            state.initial_byte..state.expected_byte,
            &state.format,
            state.pixels_per_point,
            &state.metric_identity,
        )?;
    }
    Ok(MetricGlyphBatch {
        glyphs,
        consumed_bytes: consumed,
        continuation: (!complete).then_some(state),
        status: complete
            .then_some(MetricLayoutStatus::Complete)
            .unwrap_or(MetricLayoutStatus::NeedMoreInput),
        summary,
    })
}

fn shape_metric_chars(
    font: &mut crate::text::font::Font<'_>,
    pixels_per_point: f32,
    format: &TextFormat,
    source_start_byte: u64,
    text: &str,
    line_height: f32,
    shape: &mut ShapeState,
    glyphs: &mut Vec<MetricGlyph>,
    max_glyphs: usize,
) -> usize {
    shape_metric_chars_each(
        font,
        pixels_per_point,
        format,
        source_start_byte,
        text,
        line_height,
        shape,
        max_glyphs,
        |glyph, _| {
            glyphs.push(glyph);
            true
        },
    )
}

fn shape_metric_chars_each(
    font: &mut crate::text::font::Font<'_>,
    pixels_per_point: f32,
    format: &TextFormat,
    source_start_byte: u64,
    text: &str,
    line_height: f32,
    shape: &mut ShapeState,
    max_glyphs: usize,
    mut sink: impl FnMut(MetricGlyph, ShapeState) -> bool,
) -> usize {
    let font_size = format.font_id.size;
    let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
    let mut current_font = FontFaceKey::INVALID;
    let mut current_font_face_metrics = StyledMetrics::default();
    let mut consumed = 0;
    let mut emitted = 0;
    for (offset, chr) in text.char_indices() {
        if emitted >= max_glyphs {
            break;
        }
        let (font_id, glyph_info) = font.glyph_info(chr);
        let font_face = font.fonts_by_id.get_mut(&font_id);
        if current_font != font_id {
            current_font = font_id;
            current_font_face_metrics = font_face
                .as_ref()
                .map(|face| face.styled_metrics(pixels_per_point, font_size, &format.coords))
                .unwrap_or_default();
        }
        shape.apply_kerning(
            font_face.as_deref(),
            &current_font_face_metrics,
            glyph_info.id,
            format.extra_letter_spacing,
            pixels_per_point,
        );
        let prepared = font_face.as_ref().and_then(|_| {
            FontFace::prepare_glyph_metrics(
                glyph_info,
                chr,
                &current_font_face_metrics,
                shape.cursor_x_px,
            )
        });
        let (advance_width_px, physical_x, glyph_id) = if font_face.is_some() {
            prepared
                .map(|metrics| {
                    (
                        metrics.advance_width_px,
                        metrics.physical_x,
                        metrics.glyph_id,
                    )
                })
                .unwrap_or((0.0, shape.cursor_x_px as i32, Default::default()))
        } else {
            (0.0, 0, Default::default())
        };
        let glyph = MetricGlyph {
            chr,
            source_byte_range: (source_start_byte + offset as u64)
                ..(source_start_byte + (offset + chr.len_utf8()) as u64),
            physical_x,
            logical_x: physical_x as f32 / pixels_per_point,
            advance_width: advance_width_px / pixels_per_point,
            line_height,
            font_face_height: current_font_face_metrics.row_height,
            font_face_ascent: current_font_face_metrics.ascent,
            font_height: font_metrics.row_height,
            font_ascent: font_metrics.ascent,
        };
        shape.commit_glyph(advance_width_px, glyph_id);
        let keep_going = sink(glyph, *shape);
        emitted += 1;
        consumed = offset + chr.len_utf8();
        if !keep_going {
            break;
        }
    }
    consumed
}

/// Discover bounded row descriptors for one newline-free, single-format
/// source chunk. The shaping pen remains continuous across row breaks; only
/// the row origin and candidate slots change.
pub(crate) fn layout_wrapped_row_chunk(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    metric_identity: Arc<()>,
    format: TextFormat,
    source_key: u128,
    chunk_start_byte: u64,
    chunk: &str,
    is_final_chunk: bool,
    paragraph_span: std::ops::Range<u64>,
    metric_summary: MetricLayoutSummary,
    wrap_width: f32,
    break_anywhere: bool,
    continuation: Option<WrappedRowContinuation>,
    max_output_rows: usize,
) -> Result<WrappedRowBatch, WrappedRowError> {
    const MAX_CHUNK_BYTES: usize = 96 * 1024;
    if chunk.len() > MAX_CHUNK_BYTES {
        return Err(WrappedRowError::InputChunkTooLarge);
    }
    let chunk_end = chunk_start_byte
        .checked_add(chunk.len() as u64)
        .ok_or(WrappedRowError::OffsetOverflow)?;
    if !(1..=4096).contains(&max_output_rows)
        || !pixels_per_point.is_finite()
        || pixels_per_point <= 0.0
        || wrap_width.is_nan()
        || wrap_width == f32::NEG_INFINITY
        || wrap_width < 0.0
        || !format.font_id.size.is_finite()
        || format.font_id.size <= 0.0
        || !format.extra_letter_spacing.is_finite()
        || chunk.contains('\n')
    {
        return Err(WrappedRowError::InvalidFormat);
    }
    metric_summary
        .validate_for_layout(
            source_key,
            paragraph_span.clone(),
            &format,
            pixels_per_point,
            &metric_identity,
        )
        .map_err(|_| WrappedRowError::ChangedLayoutKey)?;
    if continuation.is_none() && chunk_start_byte != paragraph_span.start {
        return Err(WrappedRowError::ChangedLayoutKey);
    }
    let fit_to_summary = metric_summary.precise_width() <= wrap_width;
    if fit_to_summary {
        if paragraph_span.start == chunk_start_byte
            && is_final_chunk
            && paragraph_span.end == chunk_end
        {
            let line_height = format.line_height.unwrap_or_else(|| {
                fonts
                    .font(&format.font_id.family)
                    .styled_metrics(pixels_per_point, format.font_id.size, &format.coords)
                    .row_height
            });
            let rows = vec![WrappedRowDescriptor {
                source_byte_range: chunk_start_byte..chunk_end,
                row_start_x: 0.0,
                width: metric_summary.precise_width(),
                line_height: PointScale::new(pixels_per_point).round_to_pixel(line_height),
                row_start_shape: ShapeState::default(),
                source_key,
                format,
                pixels_per_point,
                metric_identity,
            }];
            return Ok(WrappedRowBatch {
                rows,
                consumed_bytes: chunk.len(),
                continuation: None,
                status: WrappedRowStatus::Complete,
            });
        }
    }
    let mut state = continuation.unwrap_or_else(|| WrappedRowContinuation {
        source_key,
        expected_byte: chunk_start_byte,
        format: format.clone(),
        pixels_per_point,
        wrap_width,
        break_anywhere,
        fit_to_summary,
        metric_identity: Arc::clone(&metric_identity),
        shape: ShapeState::default(),
        row_start_shape: ShapeState::default(),
        row_start_byte: chunk_start_byte,
        row_start_x: 0.0,
        first_row_indentation: 0.0,
        candidates: std::array::from_fn(|_| None),
        pending: None,
        row_first_x: None,
        row_max_x: 0.0,
    });
    if state.source_key != source_key
        || state.expected_byte != chunk_start_byte
        || state.format != format
        || state.pixels_per_point != pixels_per_point
        || state.wrap_width != wrap_width
        || state.break_anywhere != break_anywhere
        || state.fit_to_summary != fit_to_summary
        || !Arc::ptr_eq(&state.metric_identity, &metric_identity)
    {
        return Err(WrappedRowError::ChangedLayoutKey);
    }
    let line_height = format.line_height.unwrap_or_else(|| {
        fonts
            .font(&format.font_id.family)
            .styled_metrics(pixels_per_point, format.font_id.size, &format.coords)
            .row_height
    });
    let descriptor_line_height = PointScale::new(pixels_per_point).round_to_pixel(line_height);
    let mut rows = Vec::with_capacity(max_output_rows);
    let mut pending = state.pending.take();
    let mut consumed = 0;
    let mut shape = state.shape;
    let mut font = fonts.font(&format.font_id.family);
    shape_metric_chars_each(
        &mut font,
        pixels_per_point,
        &format,
        chunk_start_byte,
        chunk,
        line_height,
        &mut shape,
        4096,
        |glyph, post| {
            if state.fit_to_summary {
                consumed = glyph
                    .source_byte_range
                    .end
                    .checked_sub(chunk_start_byte)
                    .and_then(|bytes| usize::try_from(bytes).ok())
                    .unwrap_or(0);
                return true;
            }
            let previous_was_pending = pending.is_some();
            if let Some((previous, previous_post)) = pending.take() {
                state.shape = previous_post;
                let can_continue = wrapped_row_consider_glyph(
                    &mut state,
                    previous,
                    Some(&glyph),
                    post,
                    pixels_per_point,
                    wrap_width,
                    break_anywhere,
                    descriptor_line_height,
                    &mut rows,
                    max_output_rows,
                );
                if !can_continue {
                    pending = Some((glyph, post));
                    consumed = pending.as_ref().map_or(consumed, |(glyph, _)| {
                        (glyph.source_byte_range.end - chunk_start_byte) as usize
                    });
                    return false;
                }
            }
            pending = Some((glyph, post));
            consumed = pending.as_ref().map_or(consumed, |(glyph, _)| {
                (glyph.source_byte_range.end - chunk_start_byte) as usize
            });
            !previous_was_pending || rows.len() < max_output_rows
        },
    );
    state.shape = shape;
    state.pending = pending;
    if is_final_chunk && state.fit_to_summary {
        let consumed_end = chunk_start_byte + consumed as u64;
        if consumed_end == paragraph_span.end
            && ((state.row_start_byte < consumed_end)
                || (paragraph_span.is_empty()
                    && state.row_start_byte == consumed_end
                    && rows.is_empty()))
        {
            rows.push(WrappedRowDescriptor {
                source_byte_range: paragraph_span.clone(),
                row_start_x: 0.0,
                width: metric_summary.precise_width(),
                line_height: descriptor_line_height,
                row_start_shape: ShapeState::default(),
                source_key: state.source_key,
                format: state.format.clone(),
                pixels_per_point: state.pixels_per_point,
                metric_identity: Arc::clone(&state.metric_identity),
            });
            state.row_start_byte = consumed_end;
        }
    } else if is_final_chunk && rows.len() < max_output_rows {
        if let Some((glyph, post)) = state.pending.take() {
            state.shape = post;
            let final_post = state.shape;
            wrapped_row_consider_glyph(
                &mut state,
                glyph,
                None,
                final_post,
                pixels_per_point,
                wrap_width,
                break_anywhere,
                descriptor_line_height,
                &mut rows,
                max_output_rows,
            );
        }
        let consumed_end = chunk_start_byte + consumed as u64;
        if state.row_start_byte < consumed_end && rows.len() < max_output_rows {
            rows.push(WrappedRowDescriptor {
                source_byte_range: state.row_start_byte..consumed_end,
                row_start_x: state.row_first_x.unwrap_or(state.row_start_x),
                width: state.row_first_x.map_or(0.0, |first| {
                    (state.row_max_x - state.row_start_x) - (first - state.row_start_x)
                }),
                line_height: descriptor_line_height,
                row_start_shape: state.row_start_shape,
                source_key: state.source_key,
                format: state.format.clone(),
                pixels_per_point: state.pixels_per_point,
                metric_identity: Arc::clone(&state.metric_identity),
            });
            state.row_start_byte = consumed_end;
        }
    }
    state.expected_byte = chunk_start_byte + consumed as u64;
    let consumed_end = chunk_start_byte + consumed as u64;
    // A final pending glyph can be consumed while sealing the last row and
    // fill the output budget at the same time. In that case `pending` is
    // empty, but the row beginning at `row_start_byte` still has to be
    // emitted on a subsequent empty final drain. Keep the continuation alive
    // until that residual row has a budget slot.
    let residual_row = state.row_start_byte < consumed_end;
    let blocked_by_output_budget =
        rows.len() >= max_output_rows && (state.pending.is_some() || residual_row);
    let complete = is_final_chunk && consumed == chunk.len() && !blocked_by_output_budget;
    if !chunk.is_empty() && consumed == 0 {
        return Err(WrappedRowError::NoProgress);
    }
    Ok(WrappedRowBatch {
        rows,
        consumed_bytes: consumed,
        continuation: (!complete).then_some(state),
        status: if complete {
            WrappedRowStatus::Complete
        } else {
            WrappedRowStatus::NeedMoreInput
        },
    })
}

fn wrapped_row_consider_glyph(
    state: &mut WrappedRowContinuation,
    glyph: MetricGlyph,
    next: Option<&MetricGlyph>,
    post: ShapeState,
    pixels_per_point: f32,
    wrap_width: f32,
    break_anywhere: bool,
    line_height: f32,
    rows: &mut Vec<WrappedRowDescriptor>,
    max_output_rows: usize,
) -> bool {
    let potential = glyph.logical_x + glyph.advance_width - state.row_start_x;
    let glyph_max_x = glyph.logical_x + glyph.advance_width;
    // Every live candidate also needs the extent of the suffix which would
    // become the next row if that candidate is selected.  This is constant
    // storage (six slots), and is necessary when an older candidate is chosen
    // after several already-seen glyphs.
    for candidate in &mut state.candidates {
        if let Some(candidate) = candidate {
            candidate.following_max_x = glyph_max_x;
        }
    }
    if state.row_first_x.is_none() {
        state.row_first_x = Some(glyph.logical_x);
    }
    // The ordinary oracle uses the last glyph's max_x for row size (rather
    // than a geometric maximum), which matters when spacing is negative.
    state.row_max_x = glyph_max_x;
    if wrap_width < potential {
        if state.first_row_indentation > 0.0 && !wrapped_has_good_candidate(state, break_anywhere) {
            if rows.len() >= max_output_rows {
                return false;
            }
            rows.push(WrappedRowDescriptor {
                source_byte_range: state.row_start_byte..state.row_start_byte,
                row_start_x: 0.0,
                width: 0.0,
                line_height,
                row_start_shape: state.row_start_shape,
                source_key: state.source_key,
                format: state.format.clone(),
                pixels_per_point: state.pixels_per_point,
                metric_identity: Arc::clone(&state.metric_identity),
            });
            state.row_start_x += state.first_row_indentation;
            state.first_row_indentation = 0.0;
        } else if let Some(candidate) = wrapped_take_candidate(state, break_anywhere) {
            if rows.len() >= max_output_rows {
                return false;
            }
            rows.push(WrappedRowDescriptor {
                source_byte_range: state.row_start_byte..candidate.end_byte,
                row_start_x: state.row_start_x,
                width: (candidate.last_x - state.row_start_x) + candidate.advance,
                line_height,
                row_start_shape: state.row_start_shape,
                source_key: state.source_key,
                format: state.format.clone(),
                pixels_per_point: state.pixels_per_point,
                metric_identity: Arc::clone(&state.metric_identity),
            });
            state.row_start_byte = candidate.end_byte;
            state.row_start_x = candidate.next_x;
            state.row_start_shape = candidate.post;
            // The current glyph is the first glyph considered after the
            // break only in the common case where the selected candidate is
            // immediately before it.  A candidate may be older (for example
            // after an overrun), so retain the candidate's exact next-glyph
            // origin instead of allowing the overrun glyph to redefine it.
            state.row_first_x = Some(candidate.next_x);
            state.row_max_x = candidate.following_max_x;
            wrapped_forget_candidates(state);
        }
    }
    let candidate_post = state.shape;
    wrapped_add_candidate(state, &glyph, next, pixels_per_point, candidate_post);
    state.shape = post;
    true
}

fn wrapped_has_good_candidate(state: &WrappedRowContinuation, break_anywhere: bool) -> bool {
    if break_anywhere {
        state.candidates[5].is_some()
    } else {
        state.candidates[0].is_some()
            || state.candidates[1].is_some()
            || state.candidates[2].is_some()
    }
}

fn wrapped_take_candidate(
    state: &mut WrappedRowContinuation,
    break_anywhere: bool,
) -> Option<WrappedRowCandidate> {
    let indices = if break_anywhere { [5, 5, 5] } else { [0, 1, 2] };
    let mut best = None;
    for index in indices {
        if let Some(candidate) = state.candidates[index].clone() {
            if best
                .as_ref()
                .is_none_or(|current: &WrappedRowCandidate| candidate.end_byte > current.end_byte)
            {
                best = Some(candidate);
            }
        }
    }
    if break_anywhere {
        best
    } else {
        best.or_else(|| {
            [3, 4, 5]
                .into_iter()
                .find_map(|index| state.candidates[index].clone())
        })
    }
}

fn wrapped_forget_candidates(state: &mut WrappedRowContinuation) {
    for candidate in &mut state.candidates {
        if candidate
            .as_ref()
            .is_some_and(|candidate| candidate.end_byte <= state.row_start_byte)
        {
            *candidate = None;
        }
    }
}

fn wrapped_add_candidate(
    state: &mut WrappedRowContinuation,
    glyph: &MetricGlyph,
    next: Option<&MetricGlyph>,
    _pixels_per_point: f32,
    post: ShapeState,
) {
    let index = if glyph.chr.is_whitespace() && glyph.chr != '\u{A0}' {
        Some(0)
    } else if is_cjk(glyph.chr) && (next.is_none() || is_cjk_break_allowed(next.unwrap().chr)) {
        Some(1)
    } else if glyph.chr == '-' {
        Some(3)
    } else if glyph.chr.is_ascii_punctuation() {
        Some(4)
    } else if next.is_some_and(|next| is_cjk(next.chr)) {
        Some(2)
    } else {
        None
    };
    state.candidates[5] = Some(WrappedRowCandidate {
        end_byte: glyph.source_byte_range.end,
        post,
        next_x: next.map_or(glyph.logical_x + glyph.advance_width, |next| next.logical_x),
        last_x: glyph.logical_x,
        advance: glyph.advance_width,
        following_max_x: glyph.logical_x + glyph.advance_width,
    });
    if let Some(index) = index {
        state.candidates[index] = state.candidates[5].clone();
    }
}

/// Shape one bounded, newline-free, single-format chunk while preserving the
/// exact epaint pen/kerning/allocation state between calls.
pub(crate) fn layout_unwrapped_chunk(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    font_identity: Arc<()>,
    format: TextFormat,
    source_key: u128,
    chunk_start_byte: u64,
    chunk: &str,
    is_final_chunk: bool,
    continuation: Option<UnwrappedLayoutContinuation>,
    max_output_glyphs: usize,
) -> Result<UnwrappedGlyphBatch, UnwrappedLayoutError> {
    const MAX_CHUNK_BYTES: usize = 96 * 1024;
    if chunk.len() > MAX_CHUNK_BYTES {
        return Err(UnwrappedLayoutError::InputChunkTooLarge);
    }
    let _chunk_end_byte = chunk_start_byte
        .checked_add(chunk.len() as u64)
        .ok_or(UnwrappedLayoutError::OffsetOverflow)?;
    if !(1..=4096).contains(&max_output_glyphs) {
        return Err(UnwrappedLayoutError::OutputBudgetOutOfRange);
    }
    if chunk.contains('\n')
        || !pixels_per_point.is_finite()
        || pixels_per_point <= 0.0
        || !format.font_id.size.is_finite()
        || format.font_id.size <= 0.0
        || !format.extra_letter_spacing.is_finite()
    {
        return Err(UnwrappedLayoutError::InvalidFormat);
    }

    let mut state = if let Some(state) = continuation {
        if state.source_key != source_key
            || state.expected_byte != chunk_start_byte
            || state.format != format
            || state.pixels_per_point != pixels_per_point
            || !Arc::ptr_eq(&state.font_identity, &font_identity)
        {
            return Err(UnwrappedLayoutError::ChangedLayoutKey);
        }
        state
    } else {
        UnwrappedLayoutContinuation {
            source_key,
            expected_byte: chunk_start_byte,
            format: format.clone(),
            pixels_per_point,
            font_identity,
            shape: ShapeState::default(),
            initial_byte: chunk_start_byte,
        }
    };

    let line_height = format.line_height.unwrap_or_else(|| {
        fonts
            .font(&format.font_id.family)
            .styled_metrics(pixels_per_point, format.font_id.size, &format.coords)
            .row_height
    });
    let mut glyphs = Vec::new();
    let consumed = shape_chars(
        &mut fonts.font(&format.font_id.family),
        pixels_per_point,
        &format,
        0,
        chunk,
        line_height,
        &mut state.shape,
        &mut glyphs,
        Some(max_output_glyphs),
    );
    if !chunk.is_empty() && consumed == 0 {
        return Err(UnwrappedLayoutError::NoProgress);
    }
    state.expected_byte = chunk_start_byte
        .checked_add(consumed as u64)
        .ok_or(UnwrappedLayoutError::OffsetOverflow)?;
    debug_assert!(state.expected_byte <= _chunk_end_byte);
    let complete = is_final_chunk && consumed == chunk.len();
    let summary = complete.then(|| UnwrappedLayoutSummary {
        source_key: state.source_key,
        initial_byte: state.initial_byte,
        final_byte: state.expected_byte,
        precise_advance_px: state.shape.cursor_x_px,
        format: state.format.clone(),
        pixels_per_point: state.pixels_per_point,
        font_identity: Arc::clone(&state.font_identity),
    });
    if let Some(summary) = &summary {
        summary.validate_for_layout(
            state.source_key,
            state.initial_byte..state.expected_byte,
            &state.format,
            state.pixels_per_point,
            &state.font_identity,
        )?;
    }
    Ok(UnwrappedGlyphBatch {
        glyphs,
        consumed_bytes: consumed,
        continuation: (!complete).then_some(state),
        status: complete
            .then_some(UnwrappedLayoutStatus::Complete)
            .unwrap_or(UnwrappedLayoutStatus::NeedMoreInput),
        summary,
    })
}

/// Layout text into a [`Galley`].
///
/// In most cases you should use [`crate::FontsView::layout_job`] instead
/// since that memoizes the input, making subsequent layouting of the same text much faster.
pub fn layout(fonts: &mut FontsImpl, pixels_per_point: f32, job: Arc<LayoutJob>) -> Galley {
    profiling::function_scope!();

    if job.wrap.max_rows == 0 {
        // Early-out: no text
        return Galley {
            job,
            rows: Default::default(),
            rect: Rect::ZERO,
            mesh_bounds: Rect::NOTHING,
            num_vertices: 0,
            num_indices: 0,
            pixels_per_point,
            elided: true,
            intrinsic_size: Vec2::ZERO,
        };
    }

    // For most of this we ignore the y coordinate:

    let mut paragraphs = vec![Paragraph::from_section_index(0)];
    for (section_index, section) in job.sections.iter().enumerate() {
        layout_section(
            fonts,
            pixels_per_point,
            &job,
            section_index as u32,
            section,
            &mut paragraphs,
        );
    }

    let point_scale = PointScale::new(pixels_per_point);

    let intrinsic_size = calculate_intrinsic_size(point_scale, &job, &paragraphs);

    let mut elided = false;
    let mut rows = rows_from_paragraphs(paragraphs, &job, pixels_per_point, &mut elided);
    if elided && let Some(last_placed) = rows.last_mut() {
        let last_row = Arc::make_mut(&mut last_placed.row);
        replace_last_glyph_with_overflow_character(fonts, pixels_per_point, &job, last_row);
        if let Some(last) = last_row.glyphs.last() {
            last_row.size.x = last.max_x();
        }
    }

    let justify = job.justify && job.wrap.max_width.is_finite();

    if justify || job.halign != Align::LEFT {
        let num_rows = rows.len();
        for (i, placed_row) in rows.iter_mut().enumerate() {
            let is_last_row = i + 1 == num_rows;
            let justify_row = justify && !placed_row.ends_with_newline && !is_last_row;
            halign_and_justify_row(
                point_scale,
                placed_row,
                job.halign,
                job.wrap.max_width,
                justify_row,
            );
        }
    }

    // Calculate the Y positions and tessellate the text:
    galley_from_rows(point_scale, job, rows, elided, intrinsic_size)
}

// Ignores the Y coordinate.
fn layout_section(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    job: &LayoutJob,
    section_index: u32,
    section: &LayoutSection,
    out_paragraphs: &mut Vec<Paragraph>,
) {
    let LayoutSection {
        leading_space,
        byte_range,
        format,
    } = section;
    let font_size = format.font_id.size;
    let mut font = fonts.font(&format.font_id.family);
    let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
    let line_height = section
        .format
        .line_height
        .unwrap_or(font_metrics.row_height);
    let mut paragraph = out_paragraphs.last_mut().unwrap();
    if paragraph.glyphs.is_empty() {
        paragraph.empty_paragraph_height = line_height; // TODO(emilk): replace this hack with actually including `\n` in the glyphs?
    }

    paragraph.shape.cursor_x_px += leading_space * pixels_per_point;
    // Ordinary jobs reset kerning at each section, matching the pre-streaming
    // layout behavior. Bounded continuation uses the same field across calls.
    paragraph.shape.last_glyph_id = None;
    let text = &job.text[byte_range.clone()];
    let mut segment_start = 0;
    for (offset, chr) in text.char_indices() {
        if job.break_on_newline && chr == '\n' {
            shape_chars(
                &mut font,
                pixels_per_point,
                format,
                section_index,
                &text[segment_start..offset],
                line_height,
                &mut paragraph.shape,
                &mut paragraph.glyphs,
                None,
            );
            out_paragraphs.push(Paragraph::from_section_index(section_index));
            paragraph = out_paragraphs.last_mut().unwrap();
            paragraph.empty_paragraph_height = line_height;
            segment_start = offset + chr.len_utf8();
        }
    }
    shape_chars(
        &mut font,
        pixels_per_point,
        format,
        section_index,
        &text[segment_start..],
        line_height,
        &mut paragraph.shape,
        &mut paragraph.glyphs,
        None,
    );
}

/// Shared glyph shaping loop used by ordinary layout and bounded continuation
/// layout. The caller owns row breaking; this function owns pen, kerning,
/// subpixel allocation, and observable glyph geometry.
fn shape_chars(
    font: &mut crate::text::font::Font<'_>,
    pixels_per_point: f32,
    format: &TextFormat,
    section_index: u32,
    text: &str,
    line_height: f32,
    shape: &mut ShapeState,
    glyphs: &mut Vec<Glyph>,
    max_glyphs: Option<usize>,
) -> usize {
    let font_size = format.font_id.size;
    let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
    let extra_letter_spacing = format.extra_letter_spacing;
    let mut current_font = FontFaceKey::INVALID;
    let mut current_font_face_metrics = StyledMetrics::default();
    let mut consumed = 0;

    for (offset, chr) in text.char_indices() {
        if max_glyphs.is_some_and(|limit| glyphs.len() >= limit) {
            break;
        }
        let (font_id, glyph_info) = font.glyph_info(chr);
        let mut font_face = font.fonts_by_id.get_mut(&font_id);
        if current_font != font_id {
            current_font = font_id;
            current_font_face_metrics = font_face
                .as_ref()
                .map(|font_face| {
                    font_face.styled_metrics(pixels_per_point, font_size, &format.coords)
                })
                .unwrap_or_default();
        }
        shape.apply_kerning(
            font_face.as_deref(),
            &current_font_face_metrics,
            glyph_info.id,
            extra_letter_spacing,
            pixels_per_point,
        );
        let (glyph_alloc, physical_x) = if let Some(font_face) = font_face.as_mut() {
            font_face.allocate_glyph(
                font.atlas,
                &current_font_face_metrics,
                glyph_info,
                chr,
                shape.cursor_x_px,
            )
        } else {
            Default::default()
        };
        glyphs.push(Glyph {
            chr,
            pos: pos2(physical_x as f32 / pixels_per_point, f32::NAN),
            advance_width: glyph_alloc.advance_width_px / pixels_per_point,
            line_height,
            font_face_height: current_font_face_metrics.row_height,
            font_face_ascent: current_font_face_metrics.ascent,
            font_height: font_metrics.row_height,
            font_ascent: font_metrics.ascent,
            uv_rect: glyph_alloc.uv_rect,
            section_index,
            first_vertex: 0,
        });
        shape.commit_glyph(glyph_alloc.advance_width_px, glyph_alloc.id);
        consumed = offset + chr.len_utf8();
    }
    consumed
}

/// Calculate the intrinsic size of the text.
///
/// The result is eventually passed to `Response::intrinsic_size`.
/// This works by calculating the size of each `Paragraph` (instead of each `Row`).
fn calculate_intrinsic_size(
    point_scale: PointScale,
    job: &LayoutJob,
    paragraphs: &[Paragraph],
) -> Vec2 {
    let mut intrinsic_size = Vec2::ZERO;
    for (idx, paragraph) in paragraphs.iter().enumerate() {
        // Use the precise cursor position instead of `last_glyph.max_x()`,
        // because glyph positions are pixel-snapped but the cursor tracks
        // the exact subpixel advance. This ensures that when two galleys are
        // placed side-by-side, the gap matches what it would be within a
        // single galley.
        let width = paragraph.shape.cursor_x_px / point_scale.pixels_per_point;
        intrinsic_size.x = f32::max(intrinsic_size.x, width);

        let mut height = paragraph
            .glyphs
            .iter()
            .map(|g| g.line_height)
            .max_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
            .unwrap_or(paragraph.empty_paragraph_height);
        if idx == 0 {
            height = f32::max(height, job.first_row_min_height);
        }
        intrinsic_size.y += point_scale.round_to_pixel(height);
    }
    intrinsic_size
}

// Ignores the Y coordinate.
fn rows_from_paragraphs(
    paragraphs: Vec<Paragraph>,
    job: &LayoutJob,
    pixels_per_point: f32,
    elided: &mut bool,
) -> Vec<PlacedRow> {
    let num_paragraphs = paragraphs.len();

    let mut rows = vec![];

    for (i, paragraph) in paragraphs.into_iter().enumerate() {
        if job.wrap.max_rows <= rows.len() {
            *elided = true;
            break;
        }

        let is_last_paragraph = (i + 1) == num_paragraphs;

        if paragraph.glyphs.is_empty() {
            rows.push(PlacedRow {
                pos: pos2(0.0, f32::NAN),
                row: Arc::new(Row {
                    section_index_at_start: paragraph.section_index_at_start,
                    glyphs: vec![],
                    visuals: Default::default(),
                    size: vec2(0.0, paragraph.empty_paragraph_height),
                }),
                ends_with_newline: !is_last_paragraph,
            });
        } else {
            // Use precise cursor position for width instead of pixel-snapped
            // `last_glyph.max_x()`, so that side-by-side galleys have the same
            // spacing as characters within a single galley.
            let paragraph_width = paragraph.shape.cursor_x_px / pixels_per_point;
            if paragraph_width <= job.effective_wrap_width() {
                // Early-out optimization: the whole paragraph fits on one row.
                rows.push(PlacedRow {
                    pos: pos2(0.0, f32::NAN),
                    row: Arc::new(Row {
                        section_index_at_start: paragraph.section_index_at_start,
                        glyphs: paragraph.glyphs,
                        visuals: Default::default(),
                        size: vec2(paragraph_width, 0.0),
                    }),
                    ends_with_newline: !is_last_paragraph,
                });
            } else {
                line_break(&paragraph, job, &mut rows, elided);
                let placed_row = rows.last_mut().unwrap();
                placed_row.ends_with_newline = !is_last_paragraph;
            }
        }
    }

    rows
}

fn line_break(
    paragraph: &Paragraph,
    job: &LayoutJob,
    out_rows: &mut Vec<PlacedRow>,
    elided: &mut bool,
) {
    let wrap_width = job.effective_wrap_width();

    // Keeps track of good places to insert row break if we exceed `wrap_width`.
    let mut row_break_candidates = RowBreakCandidates::default();

    let mut first_row_indentation = paragraph.glyphs[0].pos.x;
    let mut row_start_x = 0.0;
    let mut row_start_idx = 0;

    for i in 0..paragraph.glyphs.len() {
        if job.wrap.max_rows <= out_rows.len() {
            *elided = true;
            break;
        }

        let potential_row_width = paragraph.glyphs[i].max_x() - row_start_x;

        if wrap_width < potential_row_width {
            // Row break:

            if first_row_indentation > 0.0
                && !row_break_candidates.has_good_candidate(job.wrap.break_anywhere)
            {
                // Allow the first row to be completely empty, because we know there will be more space on the next row:
                // TODO(emilk): this records the height of this first row as zero, though that is probably fine since first_row_indentation usually comes with a first_row_min_height.
                out_rows.push(PlacedRow {
                    pos: pos2(0.0, f32::NAN),
                    row: Arc::new(Row {
                        section_index_at_start: paragraph.section_index_at_start,
                        glyphs: vec![],
                        visuals: Default::default(),
                        size: Vec2::ZERO,
                    }),
                    ends_with_newline: false,
                });
                row_start_x += first_row_indentation;
                first_row_indentation = 0.0;
            } else if let Some(last_kept_index) = row_break_candidates.get(job.wrap.break_anywhere)
            {
                let glyphs: Vec<Glyph> = paragraph.glyphs[row_start_idx..=last_kept_index]
                    .iter()
                    .copied()
                    .map(|mut glyph| {
                        glyph.pos.x -= row_start_x;
                        glyph
                    })
                    .collect();

                let section_index_at_start = glyphs[0].section_index;
                let paragraph_max_x = glyphs.last().unwrap().max_x();

                out_rows.push(PlacedRow {
                    pos: pos2(0.0, f32::NAN),
                    row: Arc::new(Row {
                        section_index_at_start,
                        glyphs,
                        visuals: Default::default(),
                        size: vec2(paragraph_max_x, 0.0),
                    }),
                    ends_with_newline: false,
                });

                // Start a new row:
                row_start_idx = last_kept_index + 1;
                row_start_x = paragraph.glyphs[row_start_idx].pos.x;
                row_break_candidates.forget_before_idx(row_start_idx);
            } else {
                // Found no place to break, so we have to overrun wrap_width.
            }
        }

        row_break_candidates.add(i, &paragraph.glyphs[i..]);
    }

    if row_start_idx < paragraph.glyphs.len() {
        // Final row of text:

        if job.wrap.max_rows <= out_rows.len() {
            *elided = true; // can't fit another row
        } else {
            let paragraph_min_x = paragraph.glyphs[row_start_idx].pos.x - row_start_x;
            let paragraph_max_x = paragraph.glyphs.last().unwrap().max_x() - row_start_x;

            let glyphs: Vec<Glyph> = paragraph.glyphs[row_start_idx..]
                .iter()
                .copied()
                .map(|mut glyph| {
                    glyph.pos.x -= row_start_x + paragraph_min_x;
                    glyph
                })
                .collect();

            let section_index_at_start = glyphs[0].section_index;

            out_rows.push(PlacedRow {
                pos: pos2(paragraph_min_x, 0.0),
                row: Arc::new(Row {
                    section_index_at_start,
                    glyphs,
                    visuals: Default::default(),
                    size: vec2(paragraph_max_x - paragraph_min_x, 0.0),
                }),
                ends_with_newline: false,
            });
        }
    }
}

/// Trims the last glyphs in the row and replaces it with an overflow character (e.g. `…`).
///
/// Called before we have any Y coordinates.
fn replace_last_glyph_with_overflow_character(
    fonts: &mut FontsImpl,
    pixels_per_point: f32,
    job: &LayoutJob,
    row: &mut Row,
) {
    let Some(overflow_character) = job.wrap.overflow_character else {
        return;
    };

    let mut section_index = row
        .glyphs
        .last()
        .map(|g| g.section_index)
        .unwrap_or(row.section_index_at_start);
    loop {
        let section = &job.sections[section_index as usize];
        let extra_letter_spacing = section.format.extra_letter_spacing;
        let mut font = fonts.font(&section.format.font_id.family);
        let font_size = section.format.font_id.size;

        let (font_id, glyph_info) = font.glyph_info(overflow_character);
        let mut font_face = font.fonts_by_id.get_mut(&font_id);
        let font_face_metrics = font_face
            .as_mut()
            .map(|f| f.styled_metrics(pixels_per_point, font_size, &section.format.coords))
            .unwrap_or_default();

        let overflow_glyph_x = if let Some(prev_glyph) = row.glyphs.last() {
            // Kern the overflow character properly
            let pair_kerning = font_face
                .as_mut()
                .map(|font_face| {
                    if let (Some(prev_glyph_id), Some(overflow_glyph_id)) = (
                        font_face.glyph_info(prev_glyph.chr).and_then(|g| g.id),
                        font_face.glyph_info(overflow_character).and_then(|g| g.id),
                    ) {
                        font_face.pair_kerning(&font_face_metrics, prev_glyph_id, overflow_glyph_id)
                    } else {
                        0.0
                    }
                })
                .unwrap_or_default();

            prev_glyph.max_x() + extra_letter_spacing + pair_kerning
        } else {
            0.0 // TODO(emilk): heed paragraph leading_space 😬
        };

        let replacement_glyph_width = font_face
            .as_mut()
            .and_then(|f| f.glyph_info(overflow_character))
            .map(|i| {
                i.advance_width_unscaled.0 * font_face_metrics.px_scale_factor / pixels_per_point
            })
            .unwrap_or_default();

        // Check if we're within width budget:
        if overflow_glyph_x + replacement_glyph_width <= job.effective_wrap_width()
            || row.glyphs.is_empty()
        {
            // we are done

            let (replacement_glyph_alloc, physical_x) = font_face
                .as_mut()
                .map(|f| {
                    f.allocate_glyph(
                        font.atlas,
                        &font_face_metrics,
                        glyph_info,
                        overflow_character,
                        overflow_glyph_x * pixels_per_point,
                    )
                })
                .unwrap_or_default();

            let font_metrics =
                font.styled_metrics(pixels_per_point, font_size, &section.format.coords);
            let line_height = section
                .format
                .line_height
                .unwrap_or(font_metrics.row_height);

            row.glyphs.push(Glyph {
                chr: overflow_character,
                pos: pos2(physical_x as f32 / pixels_per_point, f32::NAN),
                advance_width: replacement_glyph_alloc.advance_width_px / pixels_per_point,
                line_height,
                font_face_height: font_face_metrics.row_height,
                font_face_ascent: font_face_metrics.ascent,
                font_height: font_metrics.row_height,
                font_ascent: font_metrics.ascent,
                uv_rect: replacement_glyph_alloc.uv_rect,
                section_index,
                first_vertex: 0, // filled in later
            });
            return;
        }

        // We didn't fit - pop the last glyph and try again.
        if let Some(last_glyph) = row.glyphs.pop() {
            section_index = last_glyph.section_index;
        } else {
            section_index = row.section_index_at_start;
        }
    }
}

/// Horizontally aligned the text on a row.
///
/// Ignores the Y coordinate.
fn halign_and_justify_row(
    point_scale: PointScale,
    placed_row: &mut PlacedRow,
    halign: Align,
    wrap_width: f32,
    justify: bool,
) {
    #![expect(clippy::useless_let_if_seq)] // False positive

    let row = Arc::make_mut(&mut placed_row.row);

    if row.glyphs.is_empty() {
        return;
    }

    let num_leading_spaces = row
        .glyphs
        .iter()
        .take_while(|glyph| glyph.chr.is_whitespace())
        .count();

    let glyph_range = if num_leading_spaces == row.glyphs.len() {
        // There is only whitespace
        (0, row.glyphs.len())
    } else {
        let num_trailing_spaces = row
            .glyphs
            .iter()
            .rev()
            .take_while(|glyph| glyph.chr.is_whitespace())
            .count();

        (num_leading_spaces, row.glyphs.len() - num_trailing_spaces)
    };
    let num_glyphs_in_range = glyph_range.1 - glyph_range.0;
    assert!(num_glyphs_in_range > 0, "Should have at least one glyph");

    let original_min_x = row.glyphs[glyph_range.0].logical_rect().min.x;
    let original_max_x = row.glyphs[glyph_range.1 - 1].logical_rect().max.x;
    let original_width = original_max_x - original_min_x;

    let target_width = if justify && num_glyphs_in_range > 1 {
        wrap_width
    } else {
        original_width
    };

    let (target_min_x, target_max_x) = match halign {
        Align::LEFT => (0.0, target_width),
        Align::Center => (-target_width / 2.0, target_width / 2.0),
        Align::RIGHT => (-target_width, 0.0),
    };

    let num_spaces_in_range = row.glyphs[glyph_range.0..glyph_range.1]
        .iter()
        .filter(|glyph| glyph.chr.is_whitespace())
        .count();

    let mut extra_x_per_glyph = if num_glyphs_in_range == 1 {
        0.0
    } else {
        (target_width - original_width) / (num_glyphs_in_range as f32 - 1.0)
    };
    extra_x_per_glyph = extra_x_per_glyph.at_least(0.0); // Don't contract

    let mut extra_x_per_space = 0.0;
    if 0 < num_spaces_in_range && num_spaces_in_range < num_glyphs_in_range {
        // Add an integral number of pixels between each glyph,
        // and add the balance to the spaces:

        extra_x_per_glyph = point_scale.floor_to_pixel(extra_x_per_glyph);

        extra_x_per_space = (target_width
            - original_width
            - extra_x_per_glyph * (num_glyphs_in_range as f32 - 1.0))
            / (num_spaces_in_range as f32);
    }

    placed_row.pos.x = point_scale.round_to_pixel(target_min_x);
    let mut translate_x = -original_min_x - extra_x_per_glyph * glyph_range.0 as f32;

    for glyph in &mut row.glyphs {
        glyph.pos.x += translate_x;
        glyph.pos.x = point_scale.round_to_pixel(glyph.pos.x);
        translate_x += extra_x_per_glyph;
        if glyph.chr.is_whitespace() {
            translate_x += extra_x_per_space;
        }
    }

    // Note we ignore the leading/trailing whitespace here!
    row.size.x = target_max_x - target_min_x;
}

/// Calculate the Y positions and tessellate the text.
fn galley_from_rows(
    point_scale: PointScale,
    job: Arc<LayoutJob>,
    mut rows: Vec<PlacedRow>,
    elided: bool,
    intrinsic_size: Vec2,
) -> Galley {
    let mut first_row_min_height = job.first_row_min_height;
    let mut cursor_y = 0.0;

    for placed_row in &mut rows {
        let mut max_row_height = first_row_min_height.at_least(placed_row.height());
        let row = Arc::make_mut(&mut placed_row.row);

        first_row_min_height = 0.0;
        for glyph in &row.glyphs {
            max_row_height = max_row_height.at_least(glyph.line_height);
        }
        max_row_height = point_scale.round_to_pixel(max_row_height);

        // Now position each glyph vertically:
        for glyph in &mut row.glyphs {
            let format = &job.sections[glyph.section_index as usize].format;

            glyph.pos.y = glyph.font_face_ascent

                // Apply valign to the different in height of the entire row, and the height of this `Font`:
                + format.valign.to_factor() * (max_row_height - glyph.line_height)

                // When mixing different `FontImpl` (e.g. latin and emojis),
                // we always center the difference:
                + 0.5 * (glyph.font_height - glyph.font_face_height);

            glyph.pos.y = point_scale.round_to_pixel(glyph.pos.y);
        }

        placed_row.pos.y = cursor_y;
        row.size.y = max_row_height;

        cursor_y += max_row_height;
        cursor_y = point_scale.round_to_pixel(cursor_y); // TODO(emilk): it would be better to do the calculations in pixels instead.
    }

    let format_summary = format_summary(&job);

    let mut rect = Rect::ZERO;
    let mut mesh_bounds = Rect::NOTHING;
    let mut num_vertices = 0;
    let mut num_indices = 0;

    for placed_row in &mut rows {
        rect |= placed_row.rect();

        let row = Arc::make_mut(&mut placed_row.row);
        row.visuals = tessellate_row(point_scale, &job, &format_summary, row);

        mesh_bounds |= row.visuals.mesh_bounds.translate(placed_row.pos.to_vec2());
        num_vertices += row.visuals.mesh.vertices.len();
        num_indices += row.visuals.mesh.indices.len();

        row.section_index_at_start = u32::MAX; // No longer in use.
        for glyph in &mut row.glyphs {
            glyph.section_index = u32::MAX; // No longer in use.
        }
    }

    let mut galley = Galley {
        job,
        rows,
        elided,
        rect,
        mesh_bounds,
        num_vertices,
        num_indices,
        pixels_per_point: point_scale.pixels_per_point,
        intrinsic_size,
    };

    if galley.job.round_output_to_gui {
        galley.round_output_to_gui();
    }

    galley
}

#[derive(Default)]
struct FormatSummary {
    any_background: bool,
    any_underline: bool,
    any_strikethrough: bool,
}

fn format_summary(job: &LayoutJob) -> FormatSummary {
    let mut format_summary = FormatSummary::default();
    for section in &job.sections {
        format_summary.any_background |= section.format.background != Color32::TRANSPARENT;
        format_summary.any_underline |= section.format.underline != Stroke::NONE;
        format_summary.any_strikethrough |= section.format.strikethrough != Stroke::NONE;
    }
    format_summary
}

fn tessellate_row(
    point_scale: PointScale,
    job: &LayoutJob,
    format_summary: &FormatSummary,
    row: &mut Row,
) -> RowVisuals {
    if row.glyphs.is_empty() {
        return Default::default();
    }

    let mut mesh = Mesh::default();

    mesh.reserve_triangles(row.glyphs.len() * 2);
    mesh.reserve_vertices(row.glyphs.len() * 4);

    if format_summary.any_background {
        add_row_backgrounds(point_scale, job, row, &mut mesh);
    }

    let glyph_index_start = mesh.indices.len();
    let glyph_vertex_start = mesh.vertices.len();
    tessellate_glyphs(point_scale, job, row, &mut mesh);
    let glyph_vertex_end = mesh.vertices.len();

    if format_summary.any_underline {
        add_row_hline(point_scale, row, &mut mesh, |glyph| {
            let format = &job.sections[glyph.section_index as usize].format;
            let stroke = format.underline;
            let y = glyph.logical_rect().bottom();
            (stroke, y)
        });
    }

    if format_summary.any_strikethrough {
        add_row_hline(point_scale, row, &mut mesh, |glyph| {
            let format = &job.sections[glyph.section_index as usize].format;
            let stroke = format.strikethrough;
            let y = glyph.logical_rect().center().y;
            (stroke, y)
        });
    }

    let mesh_bounds = mesh.calc_bounds();

    RowVisuals {
        mesh,
        mesh_bounds,
        glyph_index_start,
        glyph_vertex_range: glyph_vertex_start..glyph_vertex_end,
    }
}

/// Create background for glyphs that have them.
/// Creates as few rectangular regions as possible.
fn add_row_backgrounds(point_scale: PointScale, job: &LayoutJob, row: &Row, mesh: &mut Mesh) {
    if row.glyphs.is_empty() {
        return;
    }

    let mut end_run = |start: Option<(Color32, Rect, f32)>, stop_x: f32| {
        if let Some((color, start_rect, expand)) = start {
            let rect = Rect::from_min_max(start_rect.left_top(), pos2(stop_x, start_rect.bottom()));
            let rect = rect.expand(expand);
            let rect = rect.round_to_pixels(point_scale.pixels_per_point());
            mesh.add_colored_rect(rect, color);
        }
    };

    let mut run_start = None;
    let mut last_rect = Rect::NAN;

    for glyph in &row.glyphs {
        let format = &job.sections[glyph.section_index as usize].format;
        let color = format.background;
        let rect = glyph.logical_rect();

        if color == Color32::TRANSPARENT {
            end_run(run_start.take(), last_rect.right());
        } else if let Some((existing_color, start, expand)) = run_start {
            if existing_color == color
                && start.top() == rect.top()
                && start.bottom() == rect.bottom()
                && format.expand_bg == expand
            {
                // continue the same background rectangle
            } else {
                end_run(run_start.take(), last_rect.right());
                run_start = Some((color, rect, format.expand_bg));
            }
        } else {
            run_start = Some((color, rect, format.expand_bg));
        }

        last_rect = rect;
    }

    end_run(run_start.take(), last_rect.right());
}

fn tessellate_glyphs(point_scale: PointScale, job: &LayoutJob, row: &mut Row, mesh: &mut Mesh) {
    for glyph in &mut row.glyphs {
        glyph.first_vertex = mesh.vertices.len() as u32;
        let uv_rect = glyph.uv_rect;
        if !uv_rect.is_nothing() {
            let mut left_top = glyph.pos + uv_rect.offset;
            left_top.x = point_scale.round_to_pixel(left_top.x);
            left_top.y = point_scale.round_to_pixel(left_top.y);

            let rect = Rect::from_min_max(left_top, left_top + uv_rect.size);
            let uv = Rect::from_min_max(
                pos2(uv_rect.min[0] as f32, uv_rect.min[1] as f32),
                pos2(uv_rect.max[0] as f32, uv_rect.max[1] as f32),
            );

            let format = &job.sections[glyph.section_index as usize].format;

            let color = format.color;

            if format.italics {
                let idx = mesh.vertices.len() as u32;
                mesh.add_triangle(idx, idx + 1, idx + 2);
                mesh.add_triangle(idx + 2, idx + 1, idx + 3);

                let top_offset = rect.height() * 0.25 * Vec2::X;

                mesh.vertices.push(Vertex {
                    pos: rect.left_top() + top_offset,
                    uv: uv.left_top(),
                    color,
                });
                mesh.vertices.push(Vertex {
                    pos: rect.right_top() + top_offset,
                    uv: uv.right_top(),
                    color,
                });
                mesh.vertices.push(Vertex {
                    pos: rect.left_bottom(),
                    uv: uv.left_bottom(),
                    color,
                });
                mesh.vertices.push(Vertex {
                    pos: rect.right_bottom(),
                    uv: uv.right_bottom(),
                    color,
                });
            } else {
                mesh.add_rect_with_uv(rect, uv, color);
            }
        }
    }
}

/// Add a horizontal line over a row of glyphs with a stroke and y decided by a callback.
fn add_row_hline(
    point_scale: PointScale,
    row: &Row,
    mesh: &mut Mesh,
    stroke_and_y: impl Fn(&Glyph) -> (Stroke, f32),
) {
    let mut path = crate::tessellator::Path::default(); // reusing path to avoid re-allocations.

    let mut end_line = |start: Option<(Stroke, Pos2)>, stop_x: f32| {
        if let Some((stroke, start)) = start {
            let stop = pos2(stop_x, start.y);
            path.clear();
            path.add_line_segment([start, stop]);
            let feathering = 1.0 / point_scale.pixels_per_point();
            path.stroke_open(feathering, &PathStroke::from(stroke), mesh);
        }
    };

    let mut line_start = None;
    let mut last_right_x = f32::NAN;

    for glyph in &row.glyphs {
        let (stroke, mut y) = stroke_and_y(glyph);
        stroke.round_center_to_pixel(point_scale.pixels_per_point, &mut y);

        if stroke.is_empty() {
            end_line(line_start.take(), last_right_x);
        } else if let Some((existing_stroke, start)) = line_start {
            if existing_stroke == stroke && start.y == y {
                // continue the same line
            } else {
                end_line(line_start.take(), last_right_x);
                line_start = Some((stroke, pos2(glyph.pos.x, y)));
            }
        } else {
            line_start = Some((stroke, pos2(glyph.pos.x, y)));
        }

        last_right_x = glyph.max_x();
    }

    end_line(line_start.take(), last_right_x);
}

// ----------------------------------------------------------------------------

/// Keeps track of good places to break a long row of text.
/// Will focus primarily on spaces, secondarily on things like `-`
#[derive(Clone, Copy, Default)]
struct RowBreakCandidates {
    /// Breaking at ` ` or other whitespace
    /// is always the primary candidate.
    space: Option<usize>,

    /// Logograms (single character representing a whole word) or kana (Japanese hiragana and katakana) are good candidates for line break.
    cjk: Option<usize>,

    /// Breaking anywhere before a CJK character is acceptable too.
    pre_cjk: Option<usize>,

    /// Breaking at a dash is a super-
    /// good idea.
    dash: Option<usize>,

    /// This is nicer for things like URLs, e.g. www.
    /// example.com.
    punctuation: Option<usize>,

    /// Breaking after just random character is some
    /// times necessary.
    any: Option<usize>,
}

impl RowBreakCandidates {
    fn add(&mut self, index: usize, glyphs: &[Glyph]) {
        let chr = glyphs[0].chr;
        const NON_BREAKING_SPACE: char = '\u{A0}';
        if chr.is_whitespace() && chr != NON_BREAKING_SPACE {
            self.space = Some(index);
        } else if is_cjk(chr) && (glyphs.len() == 1 || is_cjk_break_allowed(glyphs[1].chr)) {
            self.cjk = Some(index);
        } else if chr == '-' {
            self.dash = Some(index);
        } else if chr.is_ascii_punctuation() {
            self.punctuation = Some(index);
        } else if glyphs.len() > 1 && is_cjk(glyphs[1].chr) {
            self.pre_cjk = Some(index);
        }
        self.any = Some(index);
    }

    fn word_boundary(&self) -> Option<usize> {
        [self.space, self.cjk, self.pre_cjk]
            .into_iter()
            .max()
            .flatten()
    }

    fn has_good_candidate(&self, break_anywhere: bool) -> bool {
        if break_anywhere {
            self.any.is_some()
        } else {
            self.word_boundary().is_some()
        }
    }

    fn get(&self, break_anywhere: bool) -> Option<usize> {
        if break_anywhere {
            self.any
        } else {
            self.word_boundary()
                .or(self.dash)
                .or(self.punctuation)
                .or(self.any)
        }
    }

    fn forget_before_idx(&mut self, index: usize) {
        let Self {
            space,
            cjk,
            pre_cjk,
            dash,
            punctuation,
            any,
        } = self;
        if space.is_some_and(|s| s < index) {
            *space = None;
        }
        if cjk.is_some_and(|s| s < index) {
            *cjk = None;
        }
        if pre_cjk.is_some_and(|s| s < index) {
            *pre_cjk = None;
        }
        if dash.is_some_and(|s| s < index) {
            *dash = None;
        }
        if punctuation.is_some_and(|s| s < index) {
            *punctuation = None;
        }
        if any.is_some_and(|s| s < index) {
            *any = None;
        }
    }
}

// ----------------------------------------------------------------------------

#[cfg(test)]
mod tests {

    use super::{super::*, *};

    // Frozen reference copied from the pre-streaming epaint 0.34.2 shaping
    // loop. Keep this test-only path independent from `shape_chars` so the
    // continuation test can detect regressions shared by both paths.
    fn pristine_reference_glyphs(
        fonts: &mut FontsImpl,
        pixels_per_point: f32,
        format: &TextFormat,
        text: &str,
    ) -> Vec<Glyph> {
        pristine_reference_shape(fonts, pixels_per_point, format, text).0
    }

    fn pristine_reference_shape(
        fonts: &mut FontsImpl,
        pixels_per_point: f32,
        format: &TextFormat,
        text: &str,
    ) -> (Vec<Glyph>, f32) {
        let mut font = fonts.font(&format.font_id.family);
        let font_size = format.font_id.size;
        let font_metrics = font.styled_metrics(pixels_per_point, font_size, &format.coords);
        let line_height = format.line_height.unwrap_or(font_metrics.row_height);
        let mut cursor_x_px = 0.0;
        let mut last_glyph_id = None;
        let mut current_font = FontFaceKey::INVALID;
        let mut current_metrics = StyledMetrics::default();
        let mut glyphs = Vec::new();
        for chr in text.chars() {
            let (font_id, glyph_info) = font.glyph_info(chr);
            let mut font_face = font.fonts_by_id.get_mut(&font_id);
            if current_font != font_id {
                current_font = font_id;
                current_metrics = font_face
                    .as_ref()
                    .map(|face| face.styled_metrics(pixels_per_point, font_size, &format.coords))
                    .unwrap_or_default();
            }
            if let (Some(face), Some(previous), Some(glyph_id)) =
                (&font_face, last_glyph_id, glyph_info.id)
            {
                cursor_x_px += face.pair_kerning_pixels(&current_metrics, previous, glyph_id);
                cursor_x_px += format.extra_letter_spacing * pixels_per_point;
            }
            let (allocation, physical_x) = font_face
                .as_mut()
                .map(|face| {
                    face.allocate_glyph(font.atlas, &current_metrics, glyph_info, chr, cursor_x_px)
                })
                .unwrap_or_default();
            glyphs.push(Glyph {
                chr,
                pos: pos2(physical_x as f32 / pixels_per_point, f32::NAN),
                advance_width: allocation.advance_width_px / pixels_per_point,
                line_height,
                font_face_height: current_metrics.row_height,
                font_face_ascent: current_metrics.ascent,
                font_height: font_metrics.row_height,
                font_ascent: font_metrics.ascent,
                uv_rect: allocation.uv_rect,
                section_index: 0,
                first_vertex: 0,
            });
            cursor_x_px += allocation.advance_width_px;
            last_glyph_id = Some(allocation.id);
        }
        (glyphs, cursor_x_px)
    }

    fn assert_raw_glyphs_equal(actual: &[Glyph], expected: &[Glyph]) {
        assert_eq!(actual.len(), expected.len());
        for (actual, expected) in actual.iter().zip(expected) {
            assert_eq!(actual.chr, expected.chr);
            assert_eq!(actual.pos.x.to_bits(), expected.pos.x.to_bits());
            assert!(actual.pos.y.is_nan() && expected.pos.y.is_nan());
            assert_eq!(
                actual.advance_width.to_bits(),
                expected.advance_width.to_bits()
            );
            assert_eq!(actual.line_height.to_bits(), expected.line_height.to_bits());
            assert_eq!(
                actual.font_face_height.to_bits(),
                expected.font_face_height.to_bits()
            );
            assert_eq!(
                actual.font_face_ascent.to_bits(),
                expected.font_face_ascent.to_bits()
            );
            assert_eq!(actual.font_height.to_bits(), expected.font_height.to_bits());
            assert_eq!(actual.font_ascent.to_bits(), expected.font_ascent.to_bits());
            assert_eq!(actual.uv_rect, expected.uv_rect);
        }
    }

    #[test]
    fn test_zero_max_width() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job = LayoutJob::single_section("W".into(), TextFormat::default());
        layout_job.wrap.max_width = 0.0;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert_eq!(galley.rows.len(), 1);
    }

    #[test]
    fn test_truncate_with_newline() {
        // No matter where we wrap, we should be appending the newline character.

        let pixels_per_point = 1.0;

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let text_format = TextFormat {
            font_id: FontId::monospace(12.0),
            ..Default::default()
        };

        for text in ["Hello\nworld", "\nfoo"] {
            for break_anywhere in [false, true] {
                for max_width in [0.0, 5.0, 10.0, 20.0, f32::INFINITY] {
                    let mut layout_job =
                        LayoutJob::single_section(text.into(), text_format.clone());
                    layout_job.wrap.max_width = max_width;
                    layout_job.wrap.max_rows = 1;
                    layout_job.wrap.break_anywhere = break_anywhere;

                    let galley = layout(&mut fonts, pixels_per_point, layout_job.into());

                    assert!(galley.elided);
                    assert_eq!(galley.rows.len(), 1);
                    let row_text = galley.rows[0].text();
                    assert!(
                        row_text.ends_with('…'),
                        "Expected row to end with `…`, got {row_text:?} when line-breaking the text {text:?} with max_width {max_width} and break_anywhere {break_anywhere}.",
                    );
                }
            }
        }

        {
            let mut layout_job = LayoutJob::single_section("Hello\nworld".into(), text_format);
            layout_job.wrap.max_width = 50.0;
            layout_job.wrap.max_rows = 1;
            layout_job.wrap.break_anywhere = false;

            let galley = layout(&mut fonts, pixels_per_point, layout_job.into());

            assert!(galley.elided);
            assert_eq!(galley.rows.len(), 1);
            let row_text = galley.rows[0].text();
            assert_eq!(row_text, "Hello…");
        }
    }

    #[test]
    fn test_cjk() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job = LayoutJob::single_section(
            "日本語とEnglishの混在した文章".into(),
            TextFormat::default(),
        );
        layout_job.wrap.max_width = 90.0;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert_eq!(
            galley.rows.iter().map(|row| row.text()).collect::<Vec<_>>(),
            vec!["日本語と", "Englishの混在", "した文章"]
        );
    }

    #[test]
    fn test_pre_cjk() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job = LayoutJob::single_section(
            "日本語とEnglishの混在した文章".into(),
            TextFormat::default(),
        );
        layout_job.wrap.max_width = 110.0;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert_eq!(
            galley.rows.iter().map(|row| row.text()).collect::<Vec<_>>(),
            vec!["日本語とEnglish", "の混在した文章"]
        );
    }

    #[test]
    fn test_truncate_width() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut layout_job =
            LayoutJob::single_section("# DNA\nMore text".into(), TextFormat::default());
        layout_job.wrap.max_width = f32::INFINITY;
        layout_job.wrap.max_rows = 1;
        layout_job.round_output_to_gui = false;
        let galley = layout(&mut fonts, pixels_per_point, layout_job.into());
        assert!(galley.elided);
        assert_eq!(
            galley.rows.iter().map(|row| row.text()).collect::<Vec<_>>(),
            vec!["# DNA…"]
        );
        let row = &galley.rows[0];
        assert_eq!(row.pos, Pos2::ZERO);
        assert_eq!(row.rect().max.x, row.glyphs.last().unwrap().max_x());
    }

    #[test]
    fn test_truncate_with_pixels_per_point() {
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());

        for pixels_per_point in [
            0.33, 0.5, 0.67, 1.0, 1.25, 1.33, 1.5, 1.75, 2.0, 3.0, 4.0, 5.0,
        ] {
            for ch in ['W', 'A', 'n', 't', 'i'] {
                let target_width = 50.0;
                let text = (0..20).map(|_| ch).collect::<String>();

                let mut job = LayoutJob::single_section(text, TextFormat::default());
                job.wrap.max_width = target_width;
                job.wrap.max_rows = 1;
                let elided_galley = layout(&mut fonts, pixels_per_point, job.into());
                assert!(elided_galley.elided);

                let test_galley = layout(
                    &mut fonts,
                    pixels_per_point,
                    Arc::new(LayoutJob::single_section(
                        (0..elided_galley.rows[0].char_count_excluding_newline())
                            .map(|_| ch)
                            .chain(std::iter::once('…'))
                            .collect::<String>(),
                        TextFormat::default(),
                    )),
                );

                assert!(elided_galley.size().x >= 0.0);
                assert!(elided_galley.size().x <= target_width);
                assert!(test_galley.size().x > target_width);
            }
        }
    }

    #[test]
    fn test_empty_row() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());

        let font_id = FontId::default();
        let font_height = fonts
            .font(&font_id.family)
            .styled_metrics(pixels_per_point, font_id.size, &VariationCoords::default())
            .row_height;

        let job = LayoutJob::simple(String::new(), font_id, Color32::WHITE, f32::INFINITY);

        let galley = layout(&mut fonts, pixels_per_point, job.into());

        assert_eq!(galley.rows.len(), 1, "Expected one row");
        assert_eq!(
            galley.rows[0].row.glyphs.len(),
            0,
            "Expected no glyphs in the empty row"
        );
        assert_eq!(
            galley.size(),
            Vec2::new(0.0, font_height.round()),
            "Unexpected galley size"
        );
        assert_eq!(
            galley.intrinsic_size(),
            Vec2::new(0.0, font_height.round()),
            "Unexpected intrinsic size"
        );
    }

    #[test]
    fn test_end_with_newline() {
        let pixels_per_point = 1.0;
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());

        let font_id = FontId::default();
        let font_height = fonts
            .font(&font_id.family)
            .styled_metrics(pixels_per_point, font_id.size, &VariationCoords::default())
            .row_height;

        let job = LayoutJob::simple("Hi!\n".to_owned(), font_id, Color32::WHITE, f32::INFINITY);

        let galley = layout(&mut fonts, pixels_per_point, job.into());

        assert_eq!(galley.rows.len(), 2, "Expected two rows");
        assert_eq!(
            galley.rows[1].row.glyphs.len(),
            0,
            "Expected no glyphs in the empty row"
        );
        assert_eq!(
            galley.size().round(),
            Vec2::new(17.0, font_height.round() * 2.0),
            "Unexpected galley size"
        );
        assert_eq!(
            galley.intrinsic_size().round(),
            Vec2::new(17.0, font_height.round() * 2.0),
            "Unexpected intrinsic size"
        );
    }

    #[test]
    fn unwrapped_chunks_match_full_glyph_baseline_at_every_utf8_split() {
        for pixels_per_point in [1.0, 1.5, 2.0] {
            let text = "AVa\u{301}lue 前方";
            let format = TextFormat {
                font_id: FontId::monospace(14.0),
                ..Default::default()
            };
            let mut baseline_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let expected =
                pristine_reference_glyphs(&mut baseline_fonts, pixels_per_point, &format, text);

            let mut stream_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let identity = stream_fonts.layout_identity();
            for split in text
                .char_indices()
                .map(|(offset, _)| offset)
                .chain(std::iter::once(text.len()))
            {
                let mut continuation = None;
                let first = &text[..split];
                let second = &text[split..];
                let mut actual = Vec::new();
                let first_batch = layout_unwrapped_chunk(
                    &mut stream_fonts,
                    pixels_per_point,
                    Arc::clone(&identity),
                    format.clone(),
                    99,
                    0,
                    first,
                    second.is_empty(),
                    continuation,
                    4096,
                )
                .expect("valid unwrapped chunk");
                actual.extend(first_batch.glyphs);
                continuation = first_batch.continuation;
                if !second.is_empty() {
                    let second_batch = layout_unwrapped_chunk(
                        &mut stream_fonts,
                        pixels_per_point,
                        Arc::clone(&identity),
                        format.clone(),
                        99,
                        split as u64,
                        second,
                        true,
                        continuation,
                        4096,
                    )
                    .expect("valid resumed chunk");
                    actual.extend(second_batch.glyphs);
                    assert!(second_batch.continuation.is_none());
                }
                assert_raw_glyphs_equal(&actual, &expected);
            }
        }
    }

    #[test]
    fn unwrapped_chunk_budget_resumes_without_empty_progress() {
        let text = "AVAVAVAV";
        let format = TextFormat::default();
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        let mut continuation = None;
        let mut offset = 0usize;
        let mut glyph_count = 0;
        let mut actual = Vec::new();
        while offset < text.len() {
            let batch = layout_unwrapped_chunk(
                &mut fonts,
                1.5,
                Arc::clone(&identity),
                format.clone(),
                123,
                offset as u64,
                &text[offset..],
                true,
                continuation,
                2,
            )
            .expect("budgeted chunk should progress");
            assert!(batch.consumed_bytes > 0);
            assert!(batch.glyphs.len() <= 2);
            glyph_count += batch.glyphs.len();
            actual.extend(batch.glyphs);
            offset += batch.consumed_bytes;
            continuation = batch.continuation;
        }
        assert_eq!(glyph_count, text.chars().count());
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let expected = pristine_reference_glyphs(&mut baseline_fonts, 1.5, &format, text);
        assert_raw_glyphs_equal(&actual, &expected);
    }

    #[test]
    fn unwrapped_chunk_rejects_identity_offsets_formats_scales_and_limits() {
        let format = TextFormat::default();
        let seed = |fonts: &mut FontsImpl| {
            let identity = fonts.layout_identity();
            layout_unwrapped_chunk(
                fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                0,
                "A",
                false,
                None,
                1,
            )
            .expect("initial chunk")
            .continuation
            .expect("continuation")
        };

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                identity,
                format.clone(),
                18,
                1,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                identity,
                format.clone(),
                17,
                2,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        let mut changed_format = format.clone();
        changed_format.font_id.size += 1.0;
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                identity,
                changed_format,
                17,
                1,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut fonts);
        let identity = fonts.layout_identity();
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.5,
                Arc::clone(&identity),
                format.clone(),
                17,
                1,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );

        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                0.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                2,
                "V",
                true,
                None,
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::InvalidFormat
        );
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                u64::MAX,
                "A",
                true,
                None,
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::OffsetOverflow
        );
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                0,
                "A",
                true,
                None,
                0,
            )
            .unwrap_err(),
            UnwrappedLayoutError::OutputBudgetOutOfRange
        );
        let oversized = "A".repeat(96 * 1024 + 1);
        assert_eq!(
            layout_unwrapped_chunk(
                &mut fonts,
                1.0,
                Arc::clone(&identity),
                format.clone(),
                17,
                0,
                &oversized,
                true,
                None,
                1,
            )
            .unwrap_err(),
            UnwrappedLayoutError::InputChunkTooLarge
        );
        let mut replacement = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let replacement_identity = replacement.layout_identity();
        let mut original = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let continuation = seed(&mut original);
        assert_eq!(
            layout_unwrapped_chunk(
                &mut replacement,
                1.0,
                replacement_identity,
                format.clone(),
                17,
                2,
                "V",
                true,
                Some(continuation),
                2,
            )
            .unwrap_err(),
            UnwrappedLayoutError::ChangedLayoutKey
        );
    }

    #[test]
    fn unwrapped_chunk_reports_empty_final_and_budgeted_final_states() {
        let format = TextFormat::default();
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        let empty = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity,
            format.clone(),
            41,
            0,
            "",
            true,
            None,
            1,
        )
        .expect("empty final is a valid close");
        assert!(empty.glyphs.is_empty());
        assert!(empty.continuation.is_none());
        assert_eq!(empty.status, UnwrappedLayoutStatus::Complete);

        let identity = fonts.layout_identity();
        let first = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity.clone(),
            format.clone(),
            42,
            0,
            "AV",
            true,
            None,
            1,
        )
        .expect("budgeted final makes progress");
        assert_eq!(first.consumed_bytes, 1);
        assert_eq!(first.status, UnwrappedLayoutStatus::NeedMoreInput);
        assert!(first.summary.is_none());
        let second = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity,
            format.clone(),
            42,
            1,
            "V",
            true,
            first.continuation,
            1,
        )
        .expect("budgeted final resumes");
        assert_eq!(second.consumed_bytes, 1);
        assert_eq!(second.status, UnwrappedLayoutStatus::Complete);
        assert!(second.continuation.is_none());
        assert!(second.summary.is_some());

        let identity = fonts.layout_identity();
        let empty_nonfinal =
            layout_unwrapped_chunk(&mut fonts, 1.0, identity, format, 43, 0, "", false, None, 1)
                .expect("empty non-final remains resumable");
        assert_eq!(empty_nonfinal.status, UnwrappedLayoutStatus::NeedMoreInput);
        assert!(empty_nonfinal.summary.is_none());
    }

    #[test]
    fn unwrapped_checkpoint_clone_replays_utf8_seams_without_glyph_storage() {
        let text = "Á前V";
        let format = TextFormat::default();
        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        for split in text
            .char_indices()
            .map(|(offset, _)| offset)
            .chain(std::iter::once(text.len()))
        {
            let first = layout_unwrapped_chunk(
                &mut fonts,
                1.25,
                Arc::clone(&identity),
                format.clone(),
                91,
                10,
                &text[..split],
                false,
                None,
                4096,
            )
            .expect("first seam chunk");
            let checkpoint = first.continuation.expect("checkpoint at seam");
            let cloned = checkpoint.clone();
            let left = layout_unwrapped_chunk(
                &mut fonts,
                1.25,
                Arc::clone(&identity),
                format.clone(),
                91,
                10 + split as u64,
                &text[split..],
                true,
                Some(checkpoint),
                4096,
            )
            .expect("original checkpoint replay");
            let right = layout_unwrapped_chunk(
                &mut fonts,
                1.25,
                Arc::clone(&identity),
                format.clone(),
                91,
                10 + split as u64,
                &text[split..],
                true,
                Some(cloned),
                4096,
            )
            .expect("cloned checkpoint replay");
            assert_raw_glyphs_equal(&left.glyphs, &right.glyphs);
            assert_eq!(left.consumed_bytes, right.consumed_bytes);
            assert_eq!(
                left.summary.as_ref().map(|s| s.byte_span()),
                right.summary.as_ref().map(|s| s.byte_span())
            );
            assert_eq!(
                left.summary.as_ref().map(|s| s.precise_width()),
                right.summary.as_ref().map(|s| s.precise_width())
            );
        }
    }

    #[test]
    fn unwrapped_summary_reports_exact_span_and_precise_pen() {
        for (pixels_per_point, spacing) in [(1.25, -50.0), (1.75, 0.2)] {
            let text = "AVálue";
            let format = TextFormat {
                font_id: FontId::monospace(14.0),
                extra_letter_spacing: spacing,
                ..Default::default()
            };
            let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let identity = fonts.layout_identity();
            let batch = layout_unwrapped_chunk(
                &mut fonts,
                pixels_per_point,
                identity,
                format.clone(),
                700,
                37,
                text,
                true,
                None,
                4096,
            )
            .expect("complete chunk");
            let summary = batch.summary.expect("summary only on final pass");
            assert_eq!(summary.byte_span(), 37..(37 + text.len() as u64));
            assert!(summary.precise_width().is_finite());

            let mut reference_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let (_reference_glyphs, reference_pen_px) =
                pristine_reference_shape(&mut reference_fonts, pixels_per_point, &format, text);
            assert_eq!(
                summary.precise_width().to_bits(),
                (reference_pen_px / pixels_per_point).to_bits()
            );
            assert_eq!(summary.start_byte(), 37);
            assert_eq!(summary.end_byte(), 37 + text.len() as u64);
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_ok()
            );
            assert!(
                summary
                    .validate_for_layout(
                        701,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        38..(38 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            let mut changed_format = format.clone();
            changed_format.extra_letter_spacing += 1.0;
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &changed_format,
                        pixels_per_point,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point + 0.5,
                        &fonts.layout_identity(),
                    )
                    .is_err()
            );
            let replacement_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            assert!(
                summary
                    .validate_for_layout(
                        700,
                        37..(37 + text.len() as u64),
                        &format,
                        pixels_per_point,
                        &replacement_fonts.layout_identity(),
                    )
                    .is_err()
            );
            if spacing < -10.0 {
                assert!(summary.precise_width() < 0.0);
            } else {
                assert!(summary.precise_width() >= 0.0);
            }

            let mut budget_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let budget_identity = budget_fonts.layout_identity();
            let mut continuation = None;
            let mut offset = 0;
            let budget_summary = loop {
                let batch = layout_unwrapped_chunk(
                    &mut budget_fonts,
                    pixels_per_point,
                    Arc::clone(&budget_identity),
                    format.clone(),
                    700,
                    37 + offset as u64,
                    &text[offset..],
                    true,
                    continuation,
                    1,
                )
                .expect("budgeted summary chunk");
                offset += batch.consumed_bytes;
                if batch.status == UnwrappedLayoutStatus::Complete {
                    break batch.summary.expect("budgeted final summary");
                }
                continuation = batch.continuation;
            };
            assert_eq!(
                summary.precise_width().to_bits(),
                budget_summary.precise_width().to_bits()
            );
        }

        let mut fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let identity = fonts.layout_identity();
        let empty = layout_unwrapped_chunk(
            &mut fonts,
            1.0,
            identity,
            TextFormat::default(),
            701,
            42,
            "",
            true,
            None,
            1,
        )
        .expect("empty final");
        assert_eq!(
            empty.summary.expect("empty final summary").byte_span(),
            42..42
        );
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn metric_scan_matches_unwrapped_geometry_without_touching_atlas() {
        let text = "A\u{200b}\t 界😀";
        let format = TextFormat::default();
        let mut metric_owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let atlas_size = metric_owner.font_image_size();
        let atlas_before = metric_owner.image();
        let _initial_delta = metric_owner.font_image_delta();
        let metrics = {
            let mut view = metric_owner.with_pixels_per_point(1.25);
            view.layout_unwrapped_metrics_chunk(format.clone(), 900, 0, text, true, None, 4096)
                .expect("metric scan")
        };
        assert_eq!(metrics.status, MetricLayoutStatus::Complete);
        assert_eq!(metrics.consumed_bytes, text.len());
        assert_eq!(metric_owner.font_image_size(), atlas_size);
        assert_eq!(metric_owner.image(), atlas_before);
        assert!(metric_owner.font_image_delta().is_none());

        let mut ordinary_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let ordinary_identity = ordinary_fonts.layout_identity();
        let ordinary = layout_unwrapped_chunk(
            &mut ordinary_fonts,
            1.25,
            ordinary_identity,
            format,
            900,
            0,
            text,
            true,
            None,
            4096,
        )
        .expect("ordinary scan");
        assert_eq!(metrics.glyphs.len(), ordinary.glyphs.len());
        for (metric, glyph) in metrics.glyphs.iter().zip(&ordinary.glyphs) {
            assert_eq!(metric.chr, glyph.chr);
            assert_eq!(
                metric.source_byte_range.end - metric.source_byte_range.start,
                metric.chr.len_utf8() as u64
            );
            assert_eq!(metric.logical_x.to_bits(), glyph.pos.x.to_bits());
            assert_eq!(
                metric.advance_width.to_bits(),
                glyph.advance_width.to_bits()
            );
            assert_eq!(metric.line_height.to_bits(), glyph.line_height.to_bits());
        }
        assert_eq!(
            metrics.summary.as_ref().unwrap().precise_width().to_bits(),
            ordinary.summary.as_ref().unwrap().precise_width().to_bits()
        );
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn metric_continuation_survives_atlas_only_recreation() {
        let text = "first chunk then second";
        let format = TextFormat::default();
        let options = TextOptions {
            max_texture_side: 1024,
            ..TextOptions::default()
        };
        let mut owner = Fonts::new(options, FontDefinitions::default());
        {
            let chars = {
                let mut font = owner.fonts.font(&FontFamily::Monospace);
                font.characters().keys().copied().collect::<String>()
            };
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout(
                chars,
                FontId::monospace(100.0),
                crate::Color32::WHITE,
                f32::INFINITY,
            );
        }
        let first = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(
                format.clone(),
                901,
                0,
                &text[..5],
                false,
                None,
                4096,
            )
            .expect("first metric chunk")
        };
        let continuation = first.continuation.expect("continuation");
        let old_layout_identity = owner.fonts.layout_identity();
        let old_metric_identity = owner.fonts.metric_identity();
        let old_uv_continuation = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_chunk(format.clone(), 902, 0, &text[..5], false, None, 4096)
                .expect("first UV chunk")
                .continuation
                .expect("UV continuation")
        };
        assert!(owner.font_atlas_fill_ratio() > 0.8);
        owner.begin_pass(options);
        assert!(!Arc::ptr_eq(
            &old_layout_identity,
            &owner.fonts.layout_identity()
        ));
        assert!(Arc::ptr_eq(
            &old_metric_identity,
            &owner.fonts.metric_identity()
        ));
        {
            let mut view = owner.with_pixels_per_point(1.0);
            assert!(
                view.layout_unwrapped_chunk(
                    format.clone(),
                    902,
                    5,
                    &text[5..],
                    true,
                    Some(old_uv_continuation),
                    4096,
                )
                .is_err()
            );
        }
        let second = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(
                format,
                901,
                5,
                &text[5..11],
                false,
                Some(continuation),
                4096,
            )
            .expect("continued metric chunk")
        };
        let continuation = second.continuation.expect("second continuation");
        {
            let chars = {
                let mut font = owner.fonts.font(&FontFamily::Monospace);
                font.characters().keys().copied().collect::<String>()
            };
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout(
                chars,
                FontId::monospace(100.0),
                crate::Color32::WHITE,
                f32::INFINITY,
            );
        }
        assert!(owner.font_atlas_fill_ratio() > 0.8);
        let old_layout_identity = owner.fonts.layout_identity();
        let old_metric_identity = owner.fonts.metric_identity();
        owner.begin_pass(options);
        assert!(!Arc::ptr_eq(
            &old_layout_identity,
            &owner.fonts.layout_identity()
        ));
        assert!(Arc::ptr_eq(
            &old_metric_identity,
            &owner.fonts.metric_identity()
        ));
        let third = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(
                TextFormat::default(),
                901,
                11,
                &text[11..],
                true,
                Some(continuation),
                4096,
            )
            .expect("third metric chunk")
        };
        assert_eq!(third.status, MetricLayoutStatus::Complete);
        assert_eq!(
            third.summary.as_ref().unwrap().byte_span(),
            0..text.len() as u64
        );
        let mut baseline = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let baseline = {
            let mut view = baseline.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(
                TextFormat::default(),
                901,
                0,
                text,
                true,
                None,
                4096,
            )
            .expect("baseline metric chunk")
        };
        assert_eq!(
            third.summary.unwrap().precise_width().to_bits(),
            baseline.summary.unwrap().precise_width().to_bits()
        );
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn metric_scan_matches_every_utf8_seam_and_budget() {
        let text = "A界😀\t z";
        for pixels_per_point in [0.75, 1.0, 1.5, 2.0] {
            for spacing in [-1.25, 0.0, 0.5] {
                let format = TextFormat {
                    extra_letter_spacing: spacing,
                    ..TextFormat::default()
                };
                let mut baseline_owner =
                    Fonts::new(TextOptions::default(), FontDefinitions::default());
                let baseline = {
                    let mut view = baseline_owner.with_pixels_per_point(pixels_per_point);
                    view.layout_unwrapped_metrics_chunk(
                        format.clone(),
                        903,
                        0,
                        text,
                        true,
                        None,
                        4096,
                    )
                    .expect("baseline metric scan")
                };
                for split in text
                    .char_indices()
                    .map(|(offset, _)| offset)
                    .chain([text.len()])
                {
                    let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
                    let first = {
                        let mut view = owner.with_pixels_per_point(pixels_per_point);
                        view.layout_unwrapped_metrics_chunk(
                            format.clone(),
                            903,
                            0,
                            &text[..split],
                            false,
                            None,
                            1,
                        )
                        .expect("seam first metric scan")
                    };
                    let mut continuation = first.continuation;
                    let mut offset = first.consumed_bytes;
                    let mut metrics = first.glyphs;
                    while offset < text.len() {
                        let batch = {
                            let mut view = owner.with_pixels_per_point(pixels_per_point);
                            view.layout_unwrapped_metrics_chunk(
                                format.clone(),
                                903,
                                offset as u64,
                                &text[offset..],
                                true,
                                continuation,
                                1,
                            )
                            .expect("budgeted metric scan")
                        };
                        assert!(batch.consumed_bytes > 0);
                        metrics.extend(batch.glyphs);
                        offset += batch.consumed_bytes;
                        continuation = batch.continuation;
                        if continuation.is_none() {
                            assert_eq!(batch.status, MetricLayoutStatus::Complete);
                            assert_eq!(batch.summary.unwrap().byte_span(), 0..text.len() as u64);
                            break;
                        }
                    }
                    assert_eq!(offset, text.len());
                    assert_eq!(metrics.len(), baseline.glyphs.len());
                    for (actual, expected) in metrics.iter().zip(&baseline.glyphs) {
                        assert_eq!(actual.chr, expected.chr);
                        assert_eq!(actual.source_byte_range, expected.source_byte_range);
                        assert_eq!(actual.logical_x.to_bits(), expected.logical_x.to_bits());
                        assert_eq!(
                            actual.advance_width.to_bits(),
                            expected.advance_width.to_bits()
                        );
                    }
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn metric_scan_rejects_changed_keys_and_reports_empty_final() {
        let format = TextFormat::default();
        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let first = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(format.clone(), 904, 0, "ab", false, None, 1)
                .expect("key validation first chunk")
        };
        let continuation = first.continuation.expect("key continuation");
        {
            let mut view = owner.with_pixels_per_point(1.0);
            assert!(
                view.layout_unwrapped_metrics_chunk(
                    format.clone(),
                    905,
                    1,
                    "b",
                    true,
                    Some(continuation.clone()),
                    4096,
                )
                .is_err()
            );
            assert!(
                view.layout_unwrapped_metrics_chunk(
                    format.clone(),
                    904,
                    2,
                    "b",
                    true,
                    Some(continuation.clone()),
                    4096,
                )
                .is_err()
            );
            let changed_format = TextFormat {
                extra_letter_spacing: 1.0,
                ..format.clone()
            };
            assert!(
                view.layout_unwrapped_metrics_chunk(
                    changed_format,
                    904,
                    1,
                    "b",
                    true,
                    Some(continuation.clone()),
                    4096,
                )
                .is_err()
            );
        }
        {
            let mut view = owner.with_pixels_per_point(1.5);
            assert!(
                view.layout_unwrapped_metrics_chunk(
                    format.clone(),
                    904,
                    1,
                    "b",
                    true,
                    Some(continuation.clone()),
                    4096,
                )
                .is_err()
            );
        }
        let mut replacement_owner = Fonts::new(TextOptions::default(), FontDefinitions::empty());
        let mut replacement_view = replacement_owner.with_pixels_per_point(1.0);
        assert!(
            replacement_view
                .layout_unwrapped_metrics_chunk(
                    format.clone(),
                    904,
                    1,
                    "b",
                    true,
                    Some(continuation.clone()),
                    4096,
                )
                .is_err()
        );
        drop(replacement_view);
        let changed_options = TextOptions {
            max_texture_side: TextOptions::default().max_texture_side + 1024,
            ..TextOptions::default()
        };
        owner.begin_pass(changed_options);
        let mut changed_options_view = owner.with_pixels_per_point(1.0);
        assert!(
            changed_options_view
                .layout_unwrapped_metrics_chunk(
                    format.clone(),
                    904,
                    1,
                    "b",
                    true,
                    Some(continuation),
                    4096,
                )
                .is_err()
        );
        let empty = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(format, 906, 0, "", true, None, 1)
                .expect("empty final metric scan")
        };
        assert_eq!(empty.status, MetricLayoutStatus::Complete);
        assert!(empty.glyphs.is_empty());
        assert_eq!(empty.summary.unwrap().byte_span(), 0..0);
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_row_descriptors_replay_against_full_layout() {
        let text = "alpha beta gamma delta";
        let format = TextFormat::default();
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut job = LayoutJob::single_section(text.to_owned(), format.clone());
        job.wrap.max_width = 45.0;
        job.round_output_to_gui = false;
        let baseline = layout(&mut baseline_fonts, 1.0, Arc::new(job));

        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let metric_summary = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(format.clone(), 905, 0, text, true, None, 4096)
                .expect("wrapped metric summary")
                .summary
                .expect("complete wrapped metric summary")
        };
        let batch = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_wrapped_row_chunk(
                format.clone(),
                905,
                0,
                text,
                true,
                0..text.len() as u64,
                metric_summary,
                45.0,
                false,
                None,
                64,
            )
            .expect("wrapped descriptor scan")
        };
        assert_eq!(batch.status, WrappedRowStatus::Complete);
        assert!(!batch.rows.is_empty());
        assert_eq!(batch.rows.len(), baseline.rows.len());
        let mut covered = 0u64..0u64;
        let mut replayed = 0;
        for (descriptor, baseline_row) in batch.rows.iter().zip(&baseline.rows) {
            let range = descriptor.source_byte_range.clone();
            covered.end = range.end;
            let glyphs = {
                let mut view = owner.with_pixels_per_point(1.0);
                view.replay_wrapped_row_chunk(
                    descriptor,
                    range.start,
                    &text[range.start as usize..range.end as usize],
                    true,
                    None,
                    4096,
                )
                .expect("wrapped descriptor replay")
            };
            assert_eq!(glyphs.glyphs.len(), baseline_row.glyphs.len());
            for (glyph_index, (actual, expected)) in
                glyphs.glyphs.iter().zip(&baseline_row.glyphs).enumerate()
            {
                assert_eq!(actual.chr, expected.chr);
                assert_eq!(
                    actual.pos.x.to_bits(),
                    expected.pos.x.to_bits(),
                    "row range {:?}, glyph {glyph_index} {:?}: actual x={} expected x={}",
                    range,
                    actual.chr,
                    actual.pos.x,
                    expected.pos.x
                );
                assert_eq!(
                    actual.advance_width.to_bits(),
                    expected.advance_width.to_bits(),
                    "row range {:?}, glyph {glyph_index} {:?}: actual advance={} expected advance={}",
                    range,
                    actual.chr,
                    actual.advance_width,
                    expected.advance_width
                );
            }
            replayed += glyphs.glyphs.len();
        }
        assert_eq!(covered, 0..text.len() as u64);
        assert_eq!(
            replayed,
            baseline.rows.iter().map(|row| row.glyphs.len()).sum()
        );
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_positive_infinity_is_explicit_no_wrap() {
        let text = "a middle row that must remain one descriptor";
        let format = TextFormat::default();
        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let metric_summary = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_unwrapped_metrics_chunk(format.clone(), 906, 0, text, true, None, 4096)
                .expect("no-wrap metrics")
                .summary
                .expect("complete no-wrap metrics")
        };
        let batch = {
            let mut view = owner.with_pixels_per_point(1.0);
            view.layout_wrapped_row_chunk(
                format,
                906,
                0,
                text,
                true,
                0..text.len() as u64,
                metric_summary,
                f32::INFINITY,
                false,
                None,
                64,
            )
            .expect("positive infinity selects no-wrap descriptor")
        };
        assert_eq!(batch.status, WrappedRowStatus::Complete);
        assert_eq!(batch.rows.len(), 1);
        assert_eq!(batch.rows[0].source_byte_range, 0..text.len() as u64);
    }

    fn wrapped_scan_for_test(
        text: &str,
        wrap_width: f32,
        break_anywhere: bool,
        chunk_bytes: usize,
        row_budget: usize,
    ) -> (Fonts, Vec<WrappedRowDescriptor>) {
        let format = TextFormat::default();
        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let key = 0x9e37_u128;
        let mut metric_continuation = None;
        let mut metric_summary = None;
        let mut offset = 0usize;
        while offset < text.len() || (text.is_empty() && metric_summary.is_none()) {
            let mut end = (offset + chunk_bytes.min(96 * 1024)).min(text.len());
            while end > offset && !text.is_char_boundary(end) {
                end -= 1;
            }
            let final_chunk = end == text.len();
            let chunk = &text[offset..end];
            let batch = owner
                .with_pixels_per_point(1.0)
                .layout_unwrapped_metrics_chunk(
                    format.clone(),
                    key,
                    offset as u64,
                    chunk,
                    final_chunk,
                    metric_continuation,
                    4096,
                )
                .expect("metric seam");
            let consumed = batch.consumed_bytes;
            metric_continuation = batch.continuation;
            metric_summary = batch.summary;
            assert!(chunk.is_empty() || consumed == chunk.len());
            offset += consumed;
            if chunk.is_empty() {
                break;
            }
        }
        let summary = metric_summary.expect("complete metric summary");
        let mut rows = Vec::new();
        let mut continuation = None;
        let mut offset = 0usize;
        let mut drain_rounds = 0usize;
        while offset < text.len()
            || continuation.is_some()
            || (text.is_empty() && continuation.is_none())
        {
            let mut end = (offset + chunk_bytes.min(96 * 1024)).min(text.len());
            while end > offset && !text.is_char_boundary(end) {
                end -= 1;
            }
            let final_chunk = end == text.len();
            let chunk = &text[offset..end];
            let batch = owner
                .with_pixels_per_point(1.0)
                .layout_wrapped_row_chunk(
                    format.clone(),
                    key,
                    offset as u64,
                    chunk,
                    final_chunk,
                    0..text.len() as u64,
                    summary.clone(),
                    wrap_width,
                    break_anywhere,
                    continuation,
                    row_budget,
                )
                .expect("wrapped seam");
            let consumed = batch.consumed_bytes;
            rows.extend(batch.rows);
            continuation = batch.continuation;
            assert!(
                chunk.is_empty() || consumed > 0,
                "wrapped seam made no progress"
            );
            offset += consumed;
            drain_rounds += 1;
            assert!(drain_rounds < 1_000_000, "wrapped drain made no progress");
            if chunk.is_empty() {
                if continuation.is_none() {
                    break;
                }
            }
        }
        assert!(continuation.is_none(), "wrapped scan did not complete");
        (owner, rows)
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_match_oracle_at_utf8_seams_and_row_budget() {
        let text = "one deux 三四 five—six punctuation, tail";
        let (mut owner, rows) = wrapped_scan_for_test(text, 42.0, false, 5, 1);
        let format = TextFormat::default();
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut job = LayoutJob::single_section(text.to_owned(), format.clone());
        job.wrap.max_width = 42.0;
        job.round_output_to_gui = false;
        let baseline = layout(&mut baseline_fonts, 1.0, Arc::new(job));
        assert_eq!(rows.len(), baseline.rows.len());
        for (descriptor, expected) in rows.iter().zip(&baseline.rows) {
            let range = descriptor.source_byte_range();
            let actual = owner
                .with_pixels_per_point(1.0)
                .replay_wrapped_row_chunk(
                    descriptor,
                    range.start,
                    &text[range.start as usize..range.end as usize],
                    true,
                    None,
                    4096,
                )
                .expect("row replay");
            assert_eq!(actual.glyphs.len(), expected.glyphs.len());
            for (actual, expected) in actual.glyphs.iter().zip(&expected.glyphs) {
                assert_eq!(actual.chr, expected.chr);
                assert_eq!(actual.pos.x.to_bits(), expected.pos.x.to_bits());
                assert_eq!(
                    actual.advance_width.to_bits(),
                    expected.advance_width.to_bits()
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_cover_candidate_classes_and_cjk_lookahead() {
        let text = "word word-word, next 界界界 日本語 end";
        let (_, rows) = wrapped_scan_for_test(text, 35.0, false, 3, 2);
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut job = LayoutJob::single_section(text.to_owned(), TextFormat::default());
        job.wrap.max_width = 35.0;
        job.round_output_to_gui = false;
        let baseline = layout(&mut baseline_fonts, 1.0, Arc::new(job));
        assert_eq!(rows.len(), baseline.rows.len());
        assert!(rows.len() > 2);
        assert_eq!(rows.first().unwrap().source_byte_range().start, 0);
        assert_eq!(
            rows.last().unwrap().source_byte_range().end,
            text.len() as u64
        );
        for (descriptor, expected) in rows.iter().zip(&baseline.rows) {
            assert_eq!(descriptor.width().to_bits(), expected.row.size.x.to_bits());
        }
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_preserve_negative_spacing_and_precise_fit_fastpath() {
        let mut format = TextFormat::default();
        format.extra_letter_spacing = -2.0;
        let text = "negative spacing still fits";
        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let summary = owner
            .with_pixels_per_point(1.0)
            .layout_unwrapped_metrics_chunk(format.clone(), 77, 0, text, true, None, 4096)
            .expect("metric summary")
            .summary
            .expect("complete summary");
        let batch = owner
            .with_pixels_per_point(1.0)
            .layout_wrapped_row_chunk(
                format,
                77,
                0,
                text,
                true,
                0..text.len() as u64,
                summary.clone(),
                summary.precise_width() + 1.0,
                false,
                None,
                1,
            )
            .expect("fit row");
        assert_eq!(batch.status, WrappedRowStatus::Complete);
        assert_eq!(batch.rows.len(), 1);
        assert_eq!(
            batch.rows[0].width().to_bits(),
            summary.precise_width().to_bits()
        );
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_empty_paragraph_emits_one_row_across_final_drain() {
        let format = TextFormat::default();
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut job = LayoutJob::single_section(String::new(), format.clone());
        job.wrap.max_width = 100.0;
        job.round_output_to_gui = false;
        let baseline = layout(&mut baseline_fonts, 1.0, Arc::new(job));
        assert_eq!(baseline.rows.len(), 1);
        assert!(baseline.rows[0].glyphs.is_empty());

        let (mut owner, rows) = wrapped_scan_for_test("", 100.0, false, 1, 1);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source_byte_range(), 0..0);
        assert_eq!(
            rows[0].width().to_bits(),
            baseline.rows[0].row.size.x.to_bits()
        );
        assert_eq!(
            rows[0].line_height().to_bits(),
            baseline.rows[0].row.size.y.to_bits()
        );
        let replay = owner
            .with_pixels_per_point(1.0)
            .replay_wrapped_row_chunk(&rows[0], 0, "", true, None, 1)
            .expect("empty row replay");
        assert!(replay.glyphs.is_empty());

        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let summary = owner
            .with_pixels_per_point(1.0)
            .layout_unwrapped_metrics_chunk(format.clone(), 906, 0, "", true, None, 4096)
            .expect("empty metric summary")
            .summary
            .expect("empty summary");
        let first = owner
            .with_pixels_per_point(1.0)
            .layout_wrapped_row_chunk(
                format.clone(),
                906,
                0,
                "",
                false,
                0..0,
                summary.clone(),
                100.0,
                false,
                None,
                1,
            )
            .expect("empty non-final chunk");
        assert!(first.rows.is_empty());
        let second = owner
            .with_pixels_per_point(1.0)
            .layout_wrapped_row_chunk(
                format,
                906,
                0,
                "",
                true,
                0..0,
                summary,
                100.0,
                false,
                first.continuation,
                1,
            )
            .expect("empty final drain");
        assert_eq!(second.status, WrappedRowStatus::Complete);
        assert_eq!(second.rows.len(), 1);
        assert_eq!(second.rows[0].source_byte_range(), 0..0);

        let (_, nonempty_rows) = wrapped_scan_for_test("x", 100.0, false, 1, 1);
        assert_eq!(
            nonempty_rows.len(),
            1,
            "non-empty final must not gain an empty row"
        );
        assert_eq!(nonempty_rows[0].source_byte_range(), 0..1);
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_empty_row_rounds_explicit_line_height_at_scale() {
        let pixels_per_point = 1.25;
        let format = TextFormat {
            line_height: Some(17.13),
            ..TextFormat::default()
        };
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut job = LayoutJob::single_section(String::new(), format.clone());
        job.wrap.max_width = 100.0;
        job.round_output_to_gui = false;
        let baseline = layout(&mut baseline_fonts, pixels_per_point, Arc::new(job));
        let expected_height = baseline.rows[0].row.size.y;

        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let summary = owner
            .with_pixels_per_point(pixels_per_point)
            .layout_unwrapped_metrics_chunk(format.clone(), 907, 0, "", true, None, 4096)
            .expect("scaled empty metric summary")
            .summary
            .expect("scaled empty summary");
        let batch = owner
            .with_pixels_per_point(pixels_per_point)
            .layout_wrapped_row_chunk(
                format,
                907,
                0,
                "",
                true,
                0..0,
                summary,
                100.0,
                false,
                None,
                1,
            )
            .expect("scaled empty row");
        assert_eq!(batch.status, WrappedRowStatus::Complete);
        assert_eq!(batch.rows.len(), 1);
        assert_eq!(
            batch.rows[0].line_height().to_bits(),
            expected_height.to_bits()
        );
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_nonempty_row_rounds_line_height_like_oracle() {
        for pixels_per_point in [1.25, 1.5] {
            for line_height in [None, Some(17.13)] {
                let format = TextFormat {
                    line_height,
                    ..TextFormat::default()
                };
                let mut baseline_fonts =
                    FontsImpl::new(TextOptions::default(), FontDefinitions::default());
                let mut job = LayoutJob::single_section("A".to_owned(), format.clone());
                job.wrap.max_width = 100.0;
                job.round_output_to_gui = false;
                let baseline = layout(&mut baseline_fonts, pixels_per_point, Arc::new(job));

                let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
                let summary = owner
                    .with_pixels_per_point(pixels_per_point)
                    .layout_unwrapped_metrics_chunk(format.clone(), 908, 0, "A", true, None, 4096)
                    .expect("nonempty metric summary")
                    .summary
                    .expect("nonempty summary");
                let batch = owner
                    .with_pixels_per_point(pixels_per_point)
                    .layout_wrapped_row_chunk(
                        format,
                        908,
                        0,
                        "A",
                        true,
                        0..1,
                        summary,
                        100.0,
                        false,
                        None,
                        1,
                    )
                    .expect("nonempty row");
                assert_eq!(batch.rows.len(), 1);
                assert_eq!(
                    batch.rows[0].line_height().to_bits(),
                    baseline.rows[0].row.size.y.to_bits(),
                    "pixels_per_point={pixels_per_point}, line_height={line_height:?}"
                );
            }
        }
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_reject_changed_source_format_scale_and_wrap_identity() {
        let text = "identity checks across wrapped descriptors";
        let format = TextFormat::default();
        let mut owner = Fonts::new(TextOptions::default(), FontDefinitions::default());
        let summary = owner
            .with_pixels_per_point(1.0)
            .layout_unwrapped_metrics_chunk(format.clone(), 99, 0, text, true, None, 4096)
            .expect("metric summary")
            .summary
            .expect("complete summary");
        let wrong_format = TextFormat {
            extra_letter_spacing: 1.0,
            ..format.clone()
        };
        let wrong_format_result = owner.with_pixels_per_point(1.0).layout_wrapped_row_chunk(
            wrong_format,
            99,
            0,
            text,
            true,
            0..text.len() as u64,
            summary.clone(),
            30.0,
            false,
            None,
            4,
        );
        assert!(matches!(
            wrong_format_result,
            Err(WrappedRowError::ChangedLayoutKey)
        ));
        let wrong_source_result = owner.with_pixels_per_point(1.0).layout_wrapped_row_chunk(
            format.clone(),
            100,
            0,
            text,
            true,
            0..text.len() as u64,
            summary.clone(),
            30.0,
            false,
            None,
            4,
        );
        assert!(matches!(
            wrong_source_result,
            Err(WrappedRowError::ChangedLayoutKey)
        ));
        let wrong_scale_result = owner.with_pixels_per_point(2.0).layout_wrapped_row_chunk(
            format.clone(),
            99,
            0,
            text,
            true,
            0..text.len() as u64,
            summary.clone(),
            30.0,
            false,
            None,
            4,
        );
        assert!(matches!(
            wrong_scale_result,
            Err(WrappedRowError::ChangedLayoutKey)
        ));
        let mut other_options = TextOptions::default();
        other_options.max_texture_side = 1024;
        let mut other_owner = Fonts::new(other_options, FontDefinitions::default());
        let wrong_options_result = other_owner
            .with_pixels_per_point(1.0)
            .layout_wrapped_row_chunk(
                format.clone(),
                99,
                0,
                text,
                true,
                0..text.len() as u64,
                summary.clone(),
                30.0,
                false,
                None,
                4,
            );
        assert!(matches!(
            wrong_options_result,
            Err(WrappedRowError::ChangedLayoutKey)
        ));
        let valid = owner
            .with_pixels_per_point(1.0)
            .layout_wrapped_row_chunk(
                format,
                99,
                0,
                text,
                true,
                0..text.len() as u64,
                summary,
                30.0,
                false,
                None,
                4,
            )
            .expect("descriptor");
        assert!(!valid.rows.is_empty());
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_stream_large_unbreakable_source_without_row_loss() {
        let text = "x".repeat(5 * 1024 * 1024 + 17);
        let (_, rows) = wrapped_scan_for_test(&text, 100_000_000.0, false, 4096, 4096);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source_byte_range(), 0..text.len() as u64);
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_large_glyph_budget_one_drains_final_pending_row() {
        let text = "x".repeat(8193);
        let (_, rows) = wrapped_scan_for_test(&text, 0.0, false, 4096, 1);
        let mut baseline_fonts = FontsImpl::new(TextOptions::default(), FontDefinitions::default());
        let mut job = LayoutJob::single_section(text.clone(), TextFormat::default());
        job.wrap.max_width = 0.0;
        job.round_output_to_gui = false;
        let baseline = layout(&mut baseline_fonts, 1.0, Arc::new(job));
        assert_eq!(rows.len(), baseline.rows.len());
        let mut next = 0u64;
        for (descriptor, expected) in rows.iter().zip(&baseline.rows) {
            let range = descriptor.source_byte_range();
            assert_eq!(range.start, next);
            assert_eq!(range.end - range.start, expected.glyphs.len() as u64);
            assert_eq!(descriptor.width().to_bits(), expected.row.size.x.to_bits());
            next = range.end;
        }
        assert_eq!(next, text.len() as u64);
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_rows_budget_one_preserves_exact_final_boundaries() {
        for length in [1usize, 2, 4095, 4096, 4097, 8193] {
            let text = "x".repeat(length);
            let chunk_bytes = if length <= 2 { 1 } else { 4096 };
            let (mut owner, rows) = wrapped_scan_for_test(&text, 0.0, false, chunk_bytes, 1);
            let mut baseline_fonts =
                FontsImpl::new(TextOptions::default(), FontDefinitions::default());
            let mut job = LayoutJob::single_section(text.clone(), TextFormat::default());
            job.wrap.max_width = 0.0;
            job.round_output_to_gui = false;
            let baseline = layout(&mut baseline_fonts, 1.0, Arc::new(job));
            assert_eq!(rows.len(), baseline.rows.len(), "length {length}");
            for (descriptor, expected) in rows.iter().zip(&baseline.rows) {
                let range = descriptor.source_byte_range();
                assert_eq!(descriptor.width().to_bits(), expected.row.size.x.to_bits());
                let replay = owner
                    .with_pixels_per_point(1.0)
                    .replay_wrapped_row_chunk(
                        descriptor,
                        range.start,
                        &text[range.start as usize..range.end as usize],
                        true,
                        None,
                        4096,
                    )
                    .expect("exact-boundary row replay");
                assert_eq!(replay.glyphs.len(), expected.glyphs.len());
                for (actual, expected) in replay.glyphs.iter().zip(&expected.glyphs) {
                    assert_eq!(actual.pos.x.to_bits(), expected.pos.x.to_bits());
                    assert_eq!(
                        actual.advance_width.to_bits(),
                        expected.advance_width.to_bits()
                    );
                }
            }
        }
    }

    #[test]
    #[cfg(feature = "default_fonts")]
    fn wrapped_replay_restarts_after_atlas_only_reset() {
        let options = TextOptions {
            max_texture_side: 1024,
            ..TextOptions::default()
        };
        let format = TextFormat::default();
        let text = "atlas reset replay";
        let mut owner = Fonts::new(options, FontDefinitions::default());
        let summary = owner
            .with_pixels_per_point(1.0)
            .layout_unwrapped_metrics_chunk(format.clone(), 88, 0, text, true, None, 4096)
            .expect("metric summary")
            .summary
            .expect("complete summary");
        let descriptor = owner
            .with_pixels_per_point(1.0)
            .layout_wrapped_row_chunk(
                format.clone(),
                88,
                0,
                text,
                true,
                0..text.len() as u64,
                summary,
                1000.0,
                false,
                None,
                1,
            )
            .expect("descriptor")
            .rows
            .pop()
            .expect("row descriptor");

        let partial = owner
            .with_pixels_per_point(1.0)
            .replay_wrapped_row_chunk(&descriptor, 0, &text[..1], false, None, 1)
            .expect("initial non-final replay chunk");
        assert!(partial.continuation.is_some());
        assert!(matches!(
            owner.with_pixels_per_point(1.0).replay_wrapped_row_chunk(
                &descriptor,
                0,
                &(text.to_owned() + "x"),
                false,
                None,
                1,
            ),
            Err(UnwrappedLayoutError::ChangedLayoutKey)
        ));
        assert!(matches!(
            owner.with_pixels_per_point(1.0).replay_wrapped_row_chunk(
                &descriptor,
                0,
                &text[..1],
                true,
                None,
                1,
            ),
            Err(UnwrappedLayoutError::ChangedLayoutKey)
        ));

        let fill = {
            let mut font = owner.fonts.font(&FontFamily::Monospace);
            font.characters().keys().copied().collect::<String>()
        };
        let first_batch = owner
            .with_pixels_per_point(1.0)
            .replay_wrapped_row_chunk(
                &descriptor,
                descriptor.source_byte_range().start,
                text,
                true,
                None,
                1,
            )
            .expect("initial bounded replay");
        let stale_continuation = first_batch.continuation.expect("pending UV batch");
        let stale_start = descriptor.source_byte_range().start + first_batch.consumed_bytes as u64;
        let mut fill_view = owner.with_pixels_per_point(1.0);
        fill_view.layout(
            fill,
            FontId::monospace(100.0),
            crate::Color32::WHITE,
            f32::INFINITY,
        );
        assert!(owner.font_atlas_fill_ratio() > 0.8);
        owner.begin_pass(options);

        let range = descriptor.source_byte_range();
        let stale_replay = owner.with_pixels_per_point(1.0).replay_wrapped_row_chunk(
            &descriptor,
            stale_start,
            &text[stale_start as usize..range.end as usize],
            true,
            Some(stale_continuation),
            1,
        );
        assert!(matches!(
            stale_replay,
            Err(UnwrappedLayoutError::ChangedLayoutKey)
        ));
        let replay = owner
            .with_pixels_per_point(1.0)
            .replay_wrapped_row_chunk(
                &descriptor,
                range.start,
                &text[range.start as usize..range.end as usize],
                true,
                None,
                4096,
            )
            .expect("descriptor replay after atlas reset");
        assert_eq!(
            replay
                .glyphs
                .iter()
                .map(|glyph| glyph.chr)
                .collect::<String>(),
            text
        );
    }
}
