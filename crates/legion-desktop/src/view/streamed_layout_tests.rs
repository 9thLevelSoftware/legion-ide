use super::*;
use egui::{Context, FontDefinitions, FontFamily, RawInput};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use uuid::Uuid;

struct FakeSource {
    text: String,
    identity: DesktopSourceIdentity,
    max_request: std::cell::Cell<usize>,
    returned_bytes: std::cell::Cell<usize>,
    reads: std::cell::RefCell<Vec<(u64, usize)>>,
}

impl FakeSource {
    fn new(text: String, snapshot_id: u64) -> Self {
        Self {
            identity: DesktopSourceIdentity {
                buffer_id: BufferId(1),
                snapshot_id: SnapshotId(snapshot_id as u128),
                buffer_version: BufferVersion(1),
                line: 0,
                line_start_byte: 0,
                logical_end_byte: text.len() as u64,
                source_key: snapshot_id as u128,
            },
            text,
            max_request: std::cell::Cell::new(0),
            returned_bytes: std::cell::Cell::new(0),
            reads: std::cell::RefCell::new(Vec::new()),
        }
    }
}

impl DesktopLineSource for FakeSource {
    fn identity(&self, _line: usize) -> DesktopSourceIdentity {
        self.identity
    }

    fn read_chunk(
        &self,
        _line: usize,
        start_byte: u64,
        max_bytes: usize,
    ) -> Result<DesktopLineChunk, String> {
        let origin = usize::try_from(self.identity.line_start_byte)
            .map_err(|_| "origin overflow".to_string())?;
        let start = usize::try_from(start_byte)
            .ok()
            .and_then(|start| start.checked_sub(origin))
            .ok_or_else(|| "start overflow".to_string())?;
        if start > self.text.len() {
            return Err("start outside source".to_string());
        }
        self.max_request.set(self.max_request.get().max(max_bytes));
        self.reads.borrow_mut().push((start_byte, max_bytes));
        let end = (start + max_bytes.min(self.text.len() - start)).min(self.text.len());
        self.returned_bytes
            .set(self.returned_bytes.get().saturating_add(end - start));
        Ok(DesktopLineChunk {
            start_byte,
            end_byte: origin as u64 + end as u64,
            is_final: end == self.text.len(),
            text: self.text[start..end].to_owned(),
        })
    }
}

#[derive(Clone)]
struct OwnedFakeSource {
    text: Arc<String>,
    identity: DesktopSourceIdentity,
    reads: Arc<AtomicUsize>,
    lease_id: Uuid,
    fail_reads: bool,
    fail_after: Option<usize>,
}

impl OwnedFakeSource {
    fn new(text: String, snapshot_id: u64) -> Self {
        Self {
            text: Arc::new(text.clone()),
            identity: DesktopSourceIdentity {
                buffer_id: BufferId(2),
                snapshot_id: SnapshotId(snapshot_id as u128),
                buffer_version: BufferVersion(1),
                line: 0,
                line_start_byte: 0,
                logical_end_byte: text.len() as u64,
                source_key: snapshot_id as u128,
            },
            reads: Arc::new(AtomicUsize::new(0)),
            lease_id: Uuid::new_v4(),
            fail_reads: false,
            fail_after: None,
        }
    }
}

impl DesktopLineSource for OwnedFakeSource {
    fn identity(&self, _line: usize) -> DesktopSourceIdentity {
        self.identity
    }

    fn read_chunk(
        &self,
        _line: usize,
        start_byte: u64,
        max_bytes: usize,
    ) -> Result<DesktopLineChunk, String> {
        self.reads.fetch_add(1, Ordering::Relaxed);
        if self.fail_reads
            || self
                .fail_after
                .is_some_and(|limit| self.reads.load(Ordering::Relaxed) > limit)
        {
            return Err("owned source failure".to_owned());
        }
        let start = usize::try_from(start_byte).map_err(|_| "start overflow".to_owned())?;
        if start > self.text.len() {
            return Err("start outside source".to_owned());
        }
        let end = (start + max_bytes.min(self.text.len() - start)).min(self.text.len());
        Ok(DesktopLineChunk {
            start_byte,
            end_byte: end as u64,
            is_final: end == self.text.len(),
            text: self.text[start..end].to_owned(),
        })
    }

    fn owned_source(&self) -> Option<Arc<dyn DesktopLineSource + Send + Sync>> {
        Some(Arc::new(self.clone()))
    }

    fn read_lease_id(&self) -> Option<Uuid> {
        Some(self.lease_id)
    }
}

fn with_ui(context: &Context, mut f: impl FnMut(&egui::Ui)) {
    let _ = context.run_ui(RawInput::default(), |ui| f(ui));
}

fn with_ui_result<T>(context: &Context, mut f: impl FnMut(&egui::Ui) -> T) -> T {
    let mut result = None;
    let _ = context.run_ui(RawInput::default(), |ui| result = Some(f(ui)));
    result.expect("test UI callback runs")
}

#[test]
fn shaping_cursor_advance_rejects_chunk_overrun_and_integer_overflow() {
    assert_eq!(checked_chunk_advance(10, 5, 20).expect("valid advance"), 15);
    assert!(matches!(
        checked_chunk_advance(10, 11, 20),
        Err(StreamedLayoutError::InvalidSourceRange)
    ));
    assert!(matches!(
        checked_chunk_advance(u64::MAX, 1, u64::MAX),
        Err(StreamedLayoutError::InvalidSourceRange)
    ));
}

#[test]
fn source_chunk_validation_rejects_extent_limit_and_early_final() {
    let extent_mismatch = DesktopLineChunk {
        start_byte: 10,
        end_byte: 15,
        is_final: false,
        text: "x".to_owned(),
    };
    assert!(matches!(
        validate_source_chunk(&extent_mismatch, 10, 8, 20, Some(20)),
        Err(StreamedLayoutError::InvalidSourceRange)
    ));
    let over_limit = DesktopLineChunk {
        start_byte: 10,
        end_byte: 14,
        is_final: false,
        text: "abcd".to_owned(),
    };
    assert!(matches!(
        validate_source_chunk(&over_limit, 10, 3, 20, Some(20)),
        Err(StreamedLayoutError::InvalidSourceRange)
    ));
    let early_final = DesktopLineChunk {
        start_byte: 10,
        end_byte: 11,
        is_final: true,
        text: "x".to_owned(),
    };
    assert!(matches!(
        validate_source_chunk(&early_final, 10, 1, 20, Some(20)),
        Err(StreamedLayoutError::InvalidSourceRange)
    ));
    validate_source_chunk(&early_final, 10, 1, 11, None)
        .expect("row replay may finish at its descriptor range");
    let false_final_at_eof = DesktopLineChunk {
        start_byte: 19,
        end_byte: 20,
        is_final: false,
        text: "x".to_owned(),
    };
    assert!(matches!(
        validate_source_chunk(&false_final_at_eof, 19, 1, 20, Some(20)),
        Err(StreamedLayoutError::InvalidSourceRange)
    ));
}

#[test]
fn metric_engine_scans_shared_state_then_live_ui_replays_geometry() {
    fn assert_send<T: Send>() {}
    assert_send::<StreamedScanState>();

    let context = Context::default();
    let source = FakeSource::new("alpha beta gamma delta".to_owned(), 242);
    let identity = source.identity;
    with_ui(&context, |ui| {
        let format = TextFormat::default();
        let metric_snapshot = ui.fonts(|fonts| fonts.metric_snapshot());
        let mut backend = metric_snapshot.create_engine();
        let mut cache = StreamedLayoutCache::default();
        cache.ensure_key(CacheKey {
            identity,
            read_lease_id: None,
            format: format.clone(),
            pixels_per_point_bits: 1.0_f32.to_bits(),
            layout_identity_epoch: ui.fonts(|fonts| fonts.layout_identity_epoch()),
            wrap_width_bits: 40.0_f32.to_bits(),
            break_anywhere: false,
        });
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        cache
            .scan
            .scan_metrics(
                &mut backend,
                &source,
                identity,
                &format,
                &mut source_budget,
                &mut glyph_budget,
            )
            .expect("metric engine scan");
        assert_eq!(cache.scan.phase, Some(ScanPhase::Rows));
        cache.scan.visible_rows = Some(0..MAX_FRAME_ROWS);
        let mut row_budget = MAX_FRAME_ROWS;
        cache
            .scan
            .scan_rows(
                &mut backend,
                &source,
                identity,
                &format,
                40.0,
                false,
                &mut source_budget,
                &mut row_budget,
                &(0..MAX_FRAME_ROWS),
                false,
            )
            .expect("row engine scan");
        assert!(!cache.scan.rows.is_empty());

        let result = cache
            .paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format,
                    pixels_per_point: 1.0,
                    wrap_width: 40.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            )
            .expect("live UI replay");
        assert!(!result.rows.is_empty());
        assert!(result.rows.iter().any(|row| !row.mesh.vertices.is_empty()));
    });
}

#[test]
fn huge_line_completes_across_bounded_frames_without_oversized_reads() {
    let source = FakeSource::new("x".repeat(5 * 1024 * 1024 + 17), 11);
    let mut cache = StreamedLayoutCache::default();
    let context = Context::default();
    let mut complete = false;
    let frame_limit = 4 * source.text.len().div_ceil(MAX_FRAME_GLYPHS) + 64;
    for _ in 0..frame_limit {
        let mut result: Option<StreamedPaintResult> = None;
        let bytes_before = source.returned_bytes.get();
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            result = Some(
                cache
                    .paint_visible(
                        ui,
                        &source,
                        0,
                        StreamedLayoutOptions {
                            format: TextFormat::default(),
                            pixels_per_point: 1.0,
                            wrap_width: 100_000_000.0,
                            break_anywhere: false,
                            visible_rows: 0..MAX_FRAME_ROWS,
                            visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                        },
                        StreamedFrameBudget {
                            source_bytes: &mut source_budget,
                            rows: &mut row_budget,
                            glyphs: &mut glyph_budget,
                        },
                    )
                    .expect("bounded streamed frame"),
            );
        });
        assert!(source.returned_bytes.get() - bytes_before <= MAX_FRAME_SOURCE_BYTES);
        assert!(cache.scan.rows.len() <= MAX_FRAME_ROWS);
        assert!(cache
            .replay
            .values()
            .all(|state| state.glyphs.len() <= MAX_FRAME_GLYPHS));
        complete = result.expect("frame result").complete;
        if complete {
            break;
        }
    }
    assert!(
        complete,
        "large line did not complete within bounded frames"
    );
    assert!(source.max_request.get() <= SOURCE_CHUNK_BYTES);
    assert!(source.reads.borrow().iter().any(|(start, _)| *start > 0));
}

#[test]
fn sealed_rows_paint_before_rows_scan_reaches_eof() {
    let source = FakeSource::new("word ".repeat(40_000), 241);
    let mut cache = StreamedLayoutCache::default();
    let context = Context::default();
    let mut saw_early_paint = false;
    for _ in 0..2_000 {
        let before = source.returned_bytes.get();
        let mut result = None;
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            result = Some(
                cache
                    .paint_visible(
                        ui,
                        &source,
                        0,
                        StreamedLayoutOptions {
                            format: TextFormat::default(),
                            pixels_per_point: 1.0,
                            wrap_width: 40.0,
                            break_anywhere: false,
                            visible_rows: 0..MAX_FRAME_ROWS,
                            visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                        },
                        StreamedFrameBudget {
                            source_bytes: &mut source_budget,
                            rows: &mut row_budget,
                            glyphs: &mut glyph_budget,
                        },
                    )
                    .expect("sealed-row paint"),
            );
        });
        assert!(source.returned_bytes.get() - before <= MAX_FRAME_SOURCE_BYTES);
        let result = result.expect("paint result");
        saw_early_paint |= !result.rows.is_empty()
            && cache.scan.phase == Some(ScanPhase::Rows)
            && cache.scan.row_next_byte < source.identity.logical_end_byte;
        if saw_early_paint {
            assert!(!result.complete);
            break;
        }
    }
    assert!(
        saw_early_paint,
        "sealed rows never painted before EOF: phase={:?}, row_next_byte={}, logical_end={}",
        cache.scan.phase, cache.scan.row_next_byte, source.identity.logical_end_byte
    );
}

#[test]
fn wrapped_rows_report_distinct_vertical_positions() {
    let source = FakeSource::new("alpha beta gamma delta epsilon zeta".to_owned(), 12);
    let mut cache = StreamedLayoutCache::default();
    let context = Context::default();
    let mut rows = Vec::new();
    with_ui(&context, |ui| {
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        rows = cache
            .paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 30.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            )
            .expect("wrapped streamed frame")
            .rows;
    });
    assert!(rows.len() > 1);
    assert!(rows
        .windows(2)
        .all(|pair| pair[0].rect.min.y < pair[1].rect.min.y));
}

#[test]
fn snapshot_identity_change_restarts_streamed_scan() {
    let mut source = FakeSource::new("identity-bound source".to_owned(), 13);
    let mut cache = StreamedLayoutCache::default();
    let context = Context::default();
    with_ui(&context, |ui| {
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        let _ = cache.paint_visible(
            ui,
            &source,
            0,
            StreamedLayoutOptions {
                format: TextFormat::default(),
                pixels_per_point: 1.0,
                wrap_width: 100_000.0,
                break_anywhere: false,
                visible_rows: 0..MAX_FRAME_ROWS,
                visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
            },
            StreamedFrameBudget {
                source_bytes: &mut source_budget,
                rows: &mut row_budget,
                glyphs: &mut glyph_budget,
            },
        );
    });
    let before = source.reads.borrow().len();
    source.identity.snapshot_id = SnapshotId(14);
    source.identity.source_key = 14;
    with_ui(&context, |ui| {
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        let _ = cache.paint_visible(
            ui,
            &source,
            0,
            StreamedLayoutOptions {
                format: TextFormat::default(),
                pixels_per_point: 1.0,
                wrap_width: 100_000.0,
                break_anywhere: false,
                visible_rows: 0..MAX_FRAME_ROWS,
                visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
            },
            StreamedFrameBudget {
                source_bytes: &mut source_budget,
                rows: &mut row_budget,
                glyphs: &mut glyph_budget,
            },
        );
    });
    let reads = source.reads.borrow();
    assert!(reads.len() > before);
    assert_eq!(reads[before].0, 0);
}

#[test]
fn atlas_recreation_does_not_reuse_completed_replay_state() {
    let source = FakeSource::new("atlas invalidation text".to_owned(), 15);
    let mut cache = StreamedLayoutCache::default();
    let context = Context::default();
    with_ui(&context, |ui| {
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        let result = cache
            .paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 100_000.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            )
            .expect("initial replay");
        assert!(result.complete);
    });
    let reads_before = source.reads.borrow().len();
    let epoch_before = context.fonts(|fonts| fonts.layout_identity_epoch());
    let mut changed_fonts = FontDefinitions::default();
    let proportional = changed_fonts
        .families
        .get(&FontFamily::Proportional)
        .cloned()
        .expect("default proportional family");
    // A named alias changes the definitions and therefore forces a real atlas
    // recreation, while the active proportional family and its metrics remain
    // byte-for-byte unchanged for this replay.
    changed_fonts
        .families
        .insert(FontFamily::Name("unused-atlas-reset".into()), proportional);
    context.set_fonts(changed_fonts);
    let mut epoch_after = epoch_before;
    with_ui(&context, |ui| {
        epoch_after = ui.fonts(|fonts| fonts.layout_identity_epoch());
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        let result = cache
            .paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 100_000.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            )
            .expect("replay after atlas reset");
        assert!(result.complete);
    });
    assert_ne!(
        epoch_after, epoch_before,
        "font definitions must recreate atlas"
    );
    assert!(source.reads.borrow().len() > reads_before);
}

#[test]
fn streamed_rows_match_ordinary_row_count_and_widths() {
    let text = "alpha beta gamma delta epsilon".to_owned();
    let source = FakeSource::new(text.clone(), 16);
    let mut cache = StreamedLayoutCache::default();
    let context = Context::default();
    with_ui(&context, |ui| {
        let format = TextFormat::default();
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        let result = cache
            .paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: format.clone(),
                    pixels_per_point: 1.0,
                    wrap_width: 35.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            )
            .expect("ordinary parity frame");
        let mut ordinary_job = egui::text::LayoutJob::simple_format(text.clone(), format.clone());
        ordinary_job.wrap.max_width = 35.0;
        ordinary_job.wrap.break_anywhere = false;
        ordinary_job.round_output_to_gui = false;
        let ordinary = ui.fonts_mut(|fonts| fonts.layout_job(ordinary_job));
        assert!(result.complete);
        assert_eq!(
            result.rows.len(),
            ordinary.rows.len(),
            "streamed rows={:?}, ordinary widths={:?}",
            result
                .rows
                .iter()
                .map(|row| (row.source_byte_range.clone(), row.rect.width()))
                .collect::<Vec<_>>(),
            ordinary
                .rows
                .iter()
                .map(|row| row.row.size.x)
                .collect::<Vec<_>>()
        );
        for (actual, expected) in result.rows.iter().zip(&ordinary.rows) {
            let expected_screen_rect =
                egui::Rect::from_min_size(actual.rect.min, expected.row.size);
            assert_eq!(
                actual.rect.width().to_bits(),
                expected_screen_rect.width().to_bits()
            );
        }
    });
}

#[test]
fn streamed_cache_pool_retains_nine_visible_line_working_set() {
    let context = Context::default();
    let mut pool = StreamedLayoutCachePool::default();
    let mut source = FakeSource::new("long visible line".to_owned(), 77);
    with_ui(&context, |ui| {
        for line in 0..9 {
            source.identity.line = line;
            source.identity.source_key = 77 + line as u128;
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            let _ = pool
                .paint_visible(
                    ui,
                    &source,
                    line,
                    StreamedLayoutOptions {
                        format: TextFormat::default(),
                        pixels_per_point: 1.0,
                        wrap_width: 10_000.0,
                        break_anywhere: false,
                        visible_rows: 0..MAX_FRAME_ROWS,
                        visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                    },
                    StreamedFrameBudget {
                        source_bytes: &mut source_budget,
                        rows: &mut row_budget,
                        glyphs: &mut glyph_budget,
                    },
                )
                .expect("working-set streamed line");
        }
    });
    assert_eq!(pool.entries.len(), 9);
}

#[test]
fn streamed_pool_rotates_budget_across_seventeen_large_active_lines() {
    let context = Context::default();
    let mut pool = StreamedLayoutCachePool::default();
    let mut source = FakeSource::new("x".repeat(300 * 1024), 88);
    let active: Vec<_> = (0..17)
        .map(|line| DesktopSourceIdentity {
            line,
            source_key: 88 + line as u128,
            ..source.identity
        })
        .collect();
    let mut completed = HashSet::new();
    let mut previous_progress = 0_u64;
    let mut saw_progress = false;
    for frame in 0..4_000 {
        pool.begin_frame(&active);
        let before = source.returned_bytes.get();
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        with_ui(&context, |ui| {
            for line in 0..17 {
                source.identity.line = line;
                source.identity.source_key = 88 + line as u128;
                let result = pool
                    .paint_visible(
                        ui,
                        &source,
                        line,
                        StreamedLayoutOptions {
                            format: TextFormat::default(),
                            pixels_per_point: 1.0,
                            wrap_width: 100_000_000.0,
                            break_anywhere: false,
                            visible_rows: 0..MAX_FRAME_ROWS,
                            visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                        },
                        StreamedFrameBudget {
                            source_bytes: &mut source_budget,
                            rows: &mut row_budget,
                            glyphs: &mut glyph_budget,
                        },
                    )
                    .expect("bounded active line frame");
                if result.complete {
                    completed.insert(line);
                }
            }
        });
        assert!(source.returned_bytes.get() - before <= MAX_FRAME_SOURCE_BYTES);
        let progress = active
            .iter()
            .filter_map(|identity| pool.debug_state(*identity))
            .map(|state| state.metric_next_byte.saturating_add(state.row_next_byte))
            .sum::<u64>();
        if frame > 0 {
            assert!(
                progress >= previous_progress,
                "active scan progress regressed at frame {frame}: {previous_progress} -> {progress}"
            );
            saw_progress |= progress > previous_progress;
        }
        previous_progress = progress;
        if completed.len() == 17 {
            break;
        }
    }
    assert!(
        saw_progress,
        "active identities never advanced their scan cursors"
    );
    assert_eq!(
        completed.len(),
        17,
        "every active identity must make progress; states={:?}",
        active
            .iter()
            .filter_map(|identity| pool.debug_state(*identity))
            .collect::<Vec<_>>()
    );
}

#[test]
fn owned_source_uses_background_scan_without_ui_source_budget() {
    let context = Context::default();
    let source = OwnedFakeSource::new("alpha beta gamma delta".to_owned(), 901);
    let identity = source.identity;
    let mut pool = StreamedLayoutCachePool::default();
    for _ in 0..200 {
        pool.begin_frame(&[identity]);
        let mut source_budget = 0;
        let mut row_budget = 0;
        let mut glyph_budget = 0;
        with_ui(&context, |ui| {
            let _ = pool.paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 80.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
        });
        assert_eq!(source_budget, 0);
        assert_eq!(row_budget, 0);
        assert_eq!(glyph_budget, 0);
        if pool
            .debug_state(identity)
            .is_some_and(|state| state.phase == Some(2))
        {
            let reads = source.reads.load(Ordering::Relaxed);
            for _ in 0..8 {
                pool.begin_frame(&[identity]);
                with_ui(&context, |ui| {
                    let mut source_budget = 0;
                    let mut row_budget = 0;
                    let mut glyph_budget = 0;
                    let _ = pool.paint_visible(
                        ui,
                        &source,
                        0,
                        StreamedLayoutOptions {
                            format: TextFormat::default(),
                            pixels_per_point: 1.0,
                            wrap_width: 80.0,
                            break_anywhere: false,
                            visible_rows: 0..MAX_FRAME_ROWS,
                            visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                        },
                        StreamedFrameBudget {
                            source_bytes: &mut source_budget,
                            rows: &mut row_budget,
                            glyphs: &mut glyph_budget,
                        },
                    );
                });
            }
            assert_eq!(source.reads.load(Ordering::Relaxed), reads);
            return;
        }
        std::thread::yield_now();
    }
    panic!("owned source worker did not advance within bounded frames");
}

#[test]
fn terminal_owned_worker_error_settles_without_repaint_or_reread() {
    let context = Context::default();
    let mut source = OwnedFakeSource::new("error".to_owned(), 902);
    source.fail_after = Some(1);
    let identity = source.identity;
    let mut pool = StreamedLayoutCachePool::default();
    let mut settled = false;
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
    while std::time::Instant::now() < deadline {
        pool.begin_frame(&[identity]);
        let mut result: Option<Result<StreamedPaintResult, StreamedLayoutError>> = None;
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            result = Some(pool.paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 80.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            ));
        });
        if result.as_ref().is_some_and(
            |result| matches!(result, Ok(result) if !result.needs_repaint && !result.complete),
        ) {
            settled = true;
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(settled, "terminal worker error did not settle");
    let reads = source.reads.load(Ordering::Relaxed);
    for _ in 0..8 {
        pool.begin_frame(&[identity]);
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            let _ = pool.paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 80.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
        });
    }
    assert_eq!(source.reads.load(Ordering::Relaxed), reads);

    source.fail_reads = false;
    source.fail_after = None;
    source.lease_id = Uuid::new_v4();
    for _ in 0..200 {
        pool.begin_frame(&[identity]);
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            let _ = pool.paint_visible(
                ui,
                &source,
                0,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 80.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
        });
        if pool
            .debug_state(identity)
            .is_some_and(|state| state.phase == Some(2))
        {
            return;
        }
        std::thread::yield_now();
    }
    let debug = pool.debug_state(identity);
    let cache = pool.entries.get(&identity);
    panic!(
        "renewed owned source did not recover after lease identity changed: debug={debug:?} cache_phase={:?} pending={:?} error={:?} handles={} reads={}",
        cache.and_then(|cache| cache.scan.phase),
        cache.map(|cache| cache.background_pending),
        cache.and_then(|cache| cache.background_error.as_deref()),
        pool.worker_handles.len(),
        source.reads.load(Ordering::Relaxed),
    );
}

#[test]
fn streamed_byte_navigation_retains_middle_target_after_long_tail() {
    let context = Context::default();
    let mut pool = StreamedLayoutCachePool::default();
    let source = FakeSource::new("x".repeat(512 * 1024), 99);
    let identity = source.identity;
    pool.request_navigation(StreamedNavigationRequest {
        identity,
        row: StreamedRequestedRow::Byte(256 * 1024),
    });

    for _ in 0..1_000 {
        pool.begin_frame(&[identity]);
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            pool.service_one_navigation(
                ui,
                &source,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 100_000_000.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
        });
        if pool.streamed_navigation_requests().is_empty() {
            break;
        }
    }
    let rows = pool
        .navigation_rows(identity)
        .expect("middle byte navigation should complete");
    assert!(rows
        .rows
        .iter()
        .any(|row| { row.start.byte_column <= 256 * 1024 && 256 * 1024 <= row.end.byte_column }));
}

#[test]
fn streamed_byte_navigation_uses_absolute_source_origin() {
    let context = Context::default();
    let mut source = FakeSource::new("x".repeat(512 * 1024), 109);
    let origin = 8_192_u64;
    source.identity.line_start_byte = origin;
    source.identity.logical_end_byte = origin + source.text.len() as u64;
    let identity = source.identity;
    let target = 256 * 1024_u64;
    let mut pool = StreamedLayoutCachePool::default();
    pool.request_navigation(StreamedNavigationRequest {
        identity,
        row: StreamedRequestedRow::Byte(target),
    });

    for _ in 0..1_000 {
        pool.begin_frame(&[identity]);
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            pool.service_one_navigation(
                ui,
                &source,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 100_000_000.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
        });
        if pool.streamed_navigation_requests().is_empty() {
            break;
        }
    }
    let rows = pool
        .navigation_rows(identity)
        .expect("absolute-origin byte navigation should complete");
    assert!(rows.rows.iter().any(|row| {
        row.start.byte_column <= target && target <= row.end.byte_column && !row.stops.is_empty()
    }));
}

#[test]
fn streamed_byte_navigation_keeps_wrapped_predecessor_row() {
    let context = Context::default();
    let source = FakeSource::new("hello world ".repeat(400), 113);
    let identity = source.identity;
    let target = 240_u64;
    let mut pool = StreamedLayoutCachePool::default();
    pool.request_navigation(StreamedNavigationRequest {
        identity,
        row: StreamedRequestedRow::Byte(target),
    });

    for _ in 0..1_000 {
        pool.begin_frame(&[identity]);
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            pool.service_one_navigation(
                ui,
                &source,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 40.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
        });
        if pool.streamed_navigation_requests().is_empty() {
            break;
        }
    }
    let rows = pool
        .navigation_rows(identity)
        .expect("wrapped byte navigation should complete");
    let target_row = rows
        .rows
        .iter()
        .find(|row| row.start.byte_column <= target && target <= row.end.byte_column)
        .expect("navigation rows must include the target wrap row");
    assert!(
        target_row.start.byte_column > 0,
        "target must land after at least one wrapped predecessor row"
    );
    assert!(
        rows.rows
            .iter()
            .any(|row| row.end.byte_column == target_row.start.byte_column),
        "wrapped-boundary navigation must retain the predecessor row ending at {:?}",
        target_row.start.byte_column
    );
}

#[test]
fn streamed_first_index_and_last_navigation_complete_across_frames() {
    let context = Context::default();
    let source = FakeSource::new("word ".repeat(40_000), 111);
    let identity = source.identity;
    for request in [
        StreamedRequestedRow::First,
        StreamedRequestedRow::Index(20),
        StreamedRequestedRow::Last,
    ] {
        let mut pool = StreamedLayoutCachePool::default();
        pool.request_navigation(StreamedNavigationRequest {
            identity,
            row: request,
        });
        for _ in 0..1_000 {
            pool.begin_frame(&[identity]);
            with_ui(&context, |ui| {
                let mut source_budget = MAX_FRAME_SOURCE_BYTES;
                let mut row_budget = MAX_FRAME_ROWS;
                let mut glyph_budget = MAX_FRAME_GLYPHS;
                pool.service_one_navigation(
                    ui,
                    &source,
                    StreamedLayoutOptions {
                        format: TextFormat::default(),
                        pixels_per_point: 1.0,
                        wrap_width: 40.0,
                        break_anywhere: false,
                        visible_rows: 0..MAX_FRAME_ROWS,
                        visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                    },
                    StreamedFrameBudget {
                        source_bytes: &mut source_budget,
                        rows: &mut row_budget,
                        glyphs: &mut glyph_budget,
                    },
                );
            });
            if pool.streamed_navigation_requests().is_empty() {
                break;
            }
        }
        let rows = pool.navigation_rows(identity).unwrap_or_else(|| {
            panic!(
                "navigation request {:?} did not complete; state={:?}; pending={:?}",
                request,
                pool.debug_state(identity),
                pool.streamed_navigation_requests()
            )
        });
        let state = pool.debug_state(identity).expect("navigation cache state");
        assert_eq!(state.identity, identity);
        assert!(state.phase.is_some());
        assert!(state.window_reset_count <= 1_000);
        assert!(state.key_reset_count <= 1_000);
        assert!(
            state.last_error.is_none(),
            "navigation state error: {:?}",
            state.last_error
        );
        assert!(!rows.rows.is_empty());
        match request {
            StreamedRequestedRow::First => {
                assert!(rows.rows.iter().any(|row| row.row_index == Some(0)))
            }
            StreamedRequestedRow::Index(index) => assert!(rows
                .rows
                .iter()
                .any(|row| row.row_index == Some(index as u32))),
            StreamedRequestedRow::Last => {
                let last = rows.rows.iter().filter_map(|row| row.row_index).max();
                assert_eq!(
                    last,
                    rows.rows
                        .first()
                        .and_then(|row| row.row_count)
                        .map(|count| count - 1)
                );
            }
            StreamedRequestedRow::Byte(_) => unreachable!(),
        }
    }
}

#[test]
fn navigation_and_visible_paint_share_one_identity_across_frames() {
    let context = Context::default();
    let source = FakeSource::new("word ".repeat(40_000), 177);
    let identity = source.identity;
    let mut pool = StreamedLayoutCachePool::default();
    pool.request_navigation(StreamedNavigationRequest {
        identity,
        row: StreamedRequestedRow::Index(20),
    });
    let mut saw_visible_rows = false;
    // With coexistence, each cache receives at most half of the 64-row
    // frame budget. In the worst case a wrapped row can consume one source
    // byte, so this source-size bound covers every possible row plus a small
    // allowance for metrics and final replay.
    let frame_limit = source.text.len().div_ceil(MAX_FRAME_ROWS.div_ceil(2)) + 8;
    for _ in 0..frame_limit {
        pool.begin_frame(&[identity]);
        with_ui(&context, |ui| {
            let mut source_budget = MAX_FRAME_SOURCE_BYTES;
            let mut row_budget = MAX_FRAME_ROWS;
            let mut glyph_budget = MAX_FRAME_GLYPHS;
            pool.service_one_navigation(
                ui,
                &source,
                StreamedLayoutOptions {
                    format: TextFormat::default(),
                    pixels_per_point: 1.0,
                    wrap_width: 40.0,
                    break_anywhere: false,
                    visible_rows: 0..MAX_FRAME_ROWS,
                    visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                },
                StreamedFrameBudget {
                    source_bytes: &mut source_budget,
                    rows: &mut row_budget,
                    glyphs: &mut glyph_budget,
                },
            );
            let visible = pool
                .paint_visible(
                    ui,
                    &source,
                    0,
                    StreamedLayoutOptions {
                        format: TextFormat::default(),
                        pixels_per_point: 1.0,
                        wrap_width: 40.0,
                        break_anywhere: false,
                        visible_rows: 0..4,
                        visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                    },
                    StreamedFrameBudget {
                        source_bytes: &mut source_budget,
                        rows: &mut row_budget,
                        glyphs: &mut glyph_budget,
                    },
                )
                .expect("visible paint alongside navigation");
            saw_visible_rows |= !visible.rows.is_empty();
        });
        if pool.streamed_navigation_requests().is_empty() && saw_visible_rows {
            break;
        }
    }
    assert!(
        saw_visible_rows,
        "visible cache did not produce rows; visible={:?}; navigation={:?}",
        pool.visible_debug_state(identity),
        pool.debug_state(identity)
    );
    let visible_state = pool
        .visible_debug_state(identity)
        .expect("visible cache state after coexistence");
    assert!(visible_state.phase.is_some());
    assert!(visible_state.metric_next_byte > 0 || visible_state.row_next_byte > 0);
    let rows = pool
        .navigation_rows(identity)
        .expect("navigation remains available after visible painting");
    assert!(rows.rows.iter().any(|row| row.row_index == Some(20)));
}

#[test]
fn stale_navigation_identity_is_discarded_without_reusing_rows() {
    let context = Context::default();
    let mut source = FakeSource::new("stale navigation".to_owned(), 233);
    let requested = source.identity;
    let mut pool = StreamedLayoutCachePool::default();
    pool.request_navigation(StreamedNavigationRequest {
        identity: requested,
        row: StreamedRequestedRow::First,
    });
    pool.begin_frame(&[requested]);
    source.identity.snapshot_id = SnapshotId(234);
    source.identity.source_key = 234;
    with_ui(&context, |ui| {
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        pool.service_one_navigation(
            ui,
            &source,
            StreamedLayoutOptions {
                format: TextFormat::default(),
                pixels_per_point: 1.0,
                wrap_width: 40.0,
                break_anywhere: false,
                visible_rows: 0..MAX_FRAME_ROWS,
                visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
            },
            StreamedFrameBudget {
                source_bytes: &mut source_budget,
                rows: &mut row_budget,
                glyphs: &mut glyph_budget,
            },
        );
    });
    assert!(pool.streamed_navigation_requests().is_empty());
    assert!(pool.navigation_rows(requested).is_none());
}

#[test]
fn zero_source_and_glyph_budget_replays_cached_rows_without_reads() {
    let context = Context::default();
    let mut pool = StreamedLayoutCachePool::default();
    let source = FakeSource::new("cached rows".to_owned(), 211);
    let identity = source.identity;
    let options = || StreamedLayoutOptions {
        format: TextFormat::default(),
        pixels_per_point: 1.0,
        wrap_width: 100_000.0,
        break_anywhere: false,
        visible_rows: 0..MAX_FRAME_ROWS,
        visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
    };

    pool.begin_frame(&[identity]);
    let first = with_ui_result(&context, |ui| {
        let mut source_budget = MAX_FRAME_SOURCE_BYTES;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = MAX_FRAME_GLYPHS;
        pool.paint_visible(
            ui,
            &source,
            0,
            options(),
            StreamedFrameBudget {
                source_bytes: &mut source_budget,
                rows: &mut row_budget,
                glyphs: &mut glyph_budget,
            },
        )
        .expect("initial cache paint")
    });
    assert!(first.complete);
    let reads_before = source.reads.borrow().len();

    pool.begin_frame(&[identity]);
    let cached = with_ui_result(&context, |ui| {
        let mut source_budget = 0;
        let mut row_budget = MAX_FRAME_ROWS;
        let mut glyph_budget = 0;
        pool.paint_visible(
            ui,
            &source,
            0,
            options(),
            StreamedFrameBudget {
                source_bytes: &mut source_budget,
                rows: &mut row_budget,
                glyphs: &mut glyph_budget,
            },
        )
        .expect("cached paint with zero source budget")
    });
    assert!(cached.complete);
    assert!(!cached.rows.is_empty());
    assert_eq!(source.reads.borrow().len(), reads_before);
}

#[test]
fn fair_admission_rotates_zero_row_shares_across_sixty_five_active_identities() {
    let context = Context::default();
    let sources: Vec<_> = (0..65)
        .map(|index| {
            FakeSource::new(
                format!("active-{index} {}", "x".repeat(32 * 1024)),
                300 + index as u64,
            )
        })
        .collect();
    let identities: Vec<_> = sources.iter().map(|source| source.identity).collect();
    let mut pool = StreamedLayoutCachePool::default();
    let mut progress = vec![false; sources.len()];

    for _ in 0..3 {
        pool.begin_frame(&identities);
        let before = sources
            .iter()
            .map(|source| source.returned_bytes.get())
            .sum::<usize>();
        for (index, source) in sources.iter().enumerate() {
            with_ui(&context, |ui| {
                let mut source_budget = MAX_FRAME_SOURCE_BYTES;
                let mut row_budget = MAX_FRAME_ROWS;
                let mut glyph_budget = MAX_FRAME_GLYPHS;
                let _ = pool.paint_visible(
                    ui,
                    source,
                    0,
                    StreamedLayoutOptions {
                        format: TextFormat::default(),
                        pixels_per_point: 1.0,
                        wrap_width: 100_000.0,
                        break_anywhere: false,
                        visible_rows: 0..MAX_FRAME_ROWS,
                        visible_bytes: 0..MAX_FRAME_GLYPHS as u64,
                    },
                    StreamedFrameBudget {
                        source_bytes: &mut source_budget,
                        rows: &mut row_budget,
                        glyphs: &mut glyph_budget,
                    },
                );
            });
            progress[index] |= source.returned_bytes.get() > 0;
        }
        let after = sources
            .iter()
            .map(|source| source.returned_bytes.get())
            .sum::<usize>();
        assert!(after - before <= MAX_FRAME_SOURCE_BYTES);
    }
    assert!(progress.into_iter().all(|advanced| advanced));
    for identity in identities {
        let state = pool.debug_state(identity).expect("active cache state");
        assert_eq!(state.identity, identity);
        assert!(state.phase.is_some());
    }
}
