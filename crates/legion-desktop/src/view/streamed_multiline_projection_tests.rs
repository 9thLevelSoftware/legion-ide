use super::*;

use std::cell::{Cell, RefCell};

use legion_protocol::{
    BufferId, BufferVersion, ByteRange, CanonicalPath, FileFingerprint, FileId, LineWrappingPolicy,
    ProtocolTextRange, SnapshotId, TextCoordinate, Utf16Position, Utf16Range, ViewportDimensions,
    ViewportLineMetric, ViewportLineSlice, ViewportLineTruncationState, ViewportProjection,
    ViewportProjectionMode, ViewportScroll, WorkspaceId,
};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState, Shell, ShellProjectionSnapshot,
};

const LINE_BYTES: usize = 300 * 1024 + 17;
const FRAME_SOURCE_BYTES: usize = 256 * 1024;

struct MultiLineSource {
    lines: Vec<String>,
    starts: Vec<u64>,
    reads: RefCell<Vec<(usize, u64, usize)>>,
    returned_bytes: Cell<usize>,
}

impl MultiLineSource {
    fn new() -> Self {
        let lines = vec!["a".repeat(LINE_BYTES), "b".repeat(LINE_BYTES)];
        let first_end = lines[0].len() as u64;
        Self {
            lines,
            starts: vec![0, first_end + 1],
            reads: RefCell::new(Vec::new()),
            returned_bytes: Cell::new(0),
        }
    }

    fn total_returned_bytes(&self) -> usize {
        self.returned_bytes.get()
    }

    fn read_diagnostics(&self) -> String {
        let reads = self.reads.borrow();
        let mut by_line = [0usize; 2];
        let mut last = [None; 2];
        for &(line, start, requested) in reads.iter() {
            if let Some(count) = by_line.get_mut(line) {
                *count += 1;
                last[line] = Some((start, requested));
            }
        }
        format!(
            "reads={by_line:?} last={last:?} total_bytes={}",
            self.total_returned_bytes()
        )
    }
}

impl DesktopLineSource for MultiLineSource {
    fn identity(&self, line: usize) -> DesktopSourceIdentity {
        let start = self.starts[line];
        DesktopSourceIdentity {
            buffer_id: BufferId(901),
            snapshot_id: SnapshotId(902),
            buffer_version: BufferVersion(1),
            line,
            line_start_byte: start,
            logical_end_byte: start + self.lines[line].len() as u64,
            source_key: 903 + line as u128,
        }
    }

    fn read_chunk(
        &self,
        line: usize,
        start_byte: u64,
        max_bytes: usize,
    ) -> Result<DesktopLineChunk, String> {
        let text = self
            .lines
            .get(line)
            .ok_or_else(|| "line outside source".to_owned())?;
        let line_start = self.starts[line];
        let relative = start_byte
            .checked_sub(line_start)
            .ok_or_else(|| "source offset before line".to_owned())? as usize;
        if relative > text.len() {
            return Err("source offset outside line".to_owned());
        }
        let end = (relative + max_bytes).min(text.len());
        let returned = end - relative;
        self.reads.borrow_mut().push((line, start_byte, max_bytes));
        self.returned_bytes
            .set(self.returned_bytes.get().saturating_add(returned));
        Ok(DesktopLineChunk {
            start_byte,
            end_byte: line_start + end as u64,
            is_final: end == text.len(),
            text: text[relative..end].to_owned(),
        })
    }
}

fn coordinate(line: u32, byte: u64) -> TextCoordinate {
    TextCoordinate {
        line,
        character: 0,
        byte_offset: Some(byte),
        utf16_offset: Some(byte),
    }
}

fn utf16_position(line: u32, character: u64) -> Utf16Position {
    Utf16Position {
        line,
        character: character as u32,
    }
}

fn multiline_snapshot(source: &MultiLineSource) -> ShellProjectionSnapshot {
    let mut snapshot = Shell::empty("streamed multiline").projection_snapshot();
    let slices = source
        .lines
        .iter()
        .enumerate()
        .map(|(line, text)| {
            let start = source.starts[line];
            let visible_end = start + 4096;
            ViewportLineSlice {
                line_number: line as u32,
                visible_text: text[..4096].to_owned(),
                byte_range: ByteRange::new(start, visible_end),
                utf16_range: Utf16Range {
                    start: utf16_position(line as u32, 0),
                    end: utf16_position(line as u32, 4096),
                },
                chunk_hash: FileFingerprint {
                    algorithm: "test".to_owned(),
                    value: format!("streamed-{line}"),
                },
                truncation_state: ViewportLineTruncationState::Both,
            }
        })
        .collect();
    let metrics = source
        .lines
        .iter()
        .enumerate()
        .map(|(line, text)| ViewportLineMetric {
            byte_length: text.len() as u64,
            utf16_length: text.encode_utf16().count() as u64,
            line_start_byte_offset: Some(source.starts[line]),
            line_start_utf16_offset: Some(source.starts[line]),
            line_ending_width: 1,
            exact: true,
        })
        .collect();
    snapshot.active_buffer_projection = ActiveBufferProjection {
        state: ActiveBufferProjectionState::Full,
        workspace_id: Some(WorkspaceId(901)),
        buffer_id: Some(BufferId(901)),
        file_id: Some(FileId(901)),
        file_path: Some(CanonicalPath("streamed-multiline.rs".to_owned())),
        viewport: Some(ViewportProjection {
            workspace_id: WorkspaceId(901),
            buffer_id: BufferId(901),
            file_id: Some(FileId(901)),
            snapshot_id: SnapshotId(902),
            buffer_version: BufferVersion(1),
            visible_range: ProtocolTextRange {
                start: coordinate(0, 0),
                end: coordinate(1, source.starts[1] + source.lines[1].len() as u64),
            },
            selections: Vec::new(),
            cursor: coordinate(0, 0),
            cursors: vec![coordinate(0, 0)],
            cursor_affinities: Vec::new(),
            scroll: ViewportScroll {
                top_line: 0,
                left_column: 0,
            },
            dimensions: ViewportDimensions {
                width_px: 900,
                height_px: 1000,
            },
            line_wrapping_policy: LineWrappingPolicy::Off,
            wrap_column: None,
            mode: ViewportProjectionMode::Normal,
            line_slices: slices,
            line_metrics: metrics,
            decoration_spans: Vec::new(),
            fold_ranges: Vec::new(),
            semantic_token_overlays: Vec::new(),
            large_file_status: None,
            schema_version: 2,
        }),
        degraded: false,
        small_buffer_preview: None,
        dirty: false,
    };
    snapshot
}

#[test]
fn projection_streams_two_huge_lines_with_global_frame_budgets() {
    let source = MultiLineSource::new();
    let snapshot = multiline_snapshot(&source);
    let mut view = ProjectionView::new();
    let context = egui::Context::default();
    let mut completed = [false; 2];
    let mut frames = 0usize;
    let mut previous_total = 0usize;
    let mut first_font_epoch = None;
    let mut last_font_epoch = None;
    let mut font_epoch_changes = 0usize;

    while completed.iter().any(|done| !done) {
        frames += 1;
        assert!(
            frames < 4096,
            "streamed projection made no bounded progress: {}; state0={:?}; state1={:?}; pending={:?}; font_epoch_first={:?} last={:?} changes={}",
            source.read_diagnostics(),
            view.streamed_layout_cache.debug_state(source.identity(0)),
            view.streamed_layout_cache.debug_state(source.identity(1)),
            view.streamed_navigation_requests(),
            first_font_epoch,
            last_font_epoch,
            font_epoch_changes
        );
        let output = context.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(900.0, 1000.0),
                )),
                ..Default::default()
            },
            |ui| {
                let _ = view.render_with_state_and_source(
                    ui,
                    &snapshot,
                    &DesktopProjectionViewState::default(),
                    Some(&source),
                );
            },
        );
        let font_epoch = context.fonts(|fonts| fonts.layout_identity_epoch());
        if first_font_epoch.is_none() {
            first_font_epoch = Some(font_epoch);
        }
        if let Some(previous) = last_font_epoch
            && previous != font_epoch
        {
            font_epoch_changes += 1;
        }
        last_font_epoch = Some(font_epoch);
        let current_total = source.total_returned_bytes();
        assert!(
            current_total - previous_total <= FRAME_SOURCE_BYTES,
            "projection exceeded global source budget"
        );
        previous_total = current_total;
        for (line, done) in completed.iter_mut().enumerate() {
            let rows = view
                .streamed_navigation_rows(source.identity(line))
                .map(|rows| rows.rows);
            if let Some(rows) = rows {
                assert!(
                    !rows.is_empty(),
                    "completed line must expose geometry rows: line={line} identity={:?} visible={:?} navigation={:?} reads={}",
                    source.identity(line),
                    view.streamed_layout_cache
                        .visible_debug_state(source.identity(line)),
                    view.streamed_layout_cache
                        .debug_state(source.identity(line)),
                    source.read_diagnostics()
                );
                assert!(
                    rows.iter().all(|row| !row.stops.is_empty()),
                    "completed line must expose navigation stops"
                );
                *done = true;
            }
        }
        let _ = output;
    }
    assert!(frames > 1, "large lines must require multiple frames");
    assert!(source.total_returned_bytes() >= 2 * LINE_BYTES);
}
