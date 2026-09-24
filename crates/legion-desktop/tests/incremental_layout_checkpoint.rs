//! Source-only experiment for a bounded layout checkpoint.
//!
//! The phase-carry cases below are research counterexamples only. They record
//! the current vendor behavior and are not production completion evidence.
//!
//! The first job is deliberately treated as provisional in its final row. The
//! next job restarts at that row's absolute logical start, so a word-break
//! decision that needs later glyphs cannot leak into the committed prefix.

use egui::{Context, Galley};
use legion_desktop::view::{DesktopCodeLineViewModel, editor_galley_for_geometry};
use legion_protocol::{ByteRange, Utf16Position, Utf16Range, ViewportLineTruncationState};

fn with_ui<T>(mut f: impl FnMut(&egui::Ui) -> T) -> T {
    let context = Context::default();
    let mut result = None;
    let _ = context.run_ui(egui::RawInput::default(), |ui| {
        result = Some(f(ui));
    });
    result.expect("test panel should run")
}

fn shape(ctx: &Context, text: &str, width: f32) -> std::sync::Arc<Galley> {
    let utf16_len = text.encode_utf16().count() as u32;
    let line = DesktopCodeLineViewModel {
        number: 1,
        text: text.to_owned(),
        highlights: Vec::new(),
        truncation_state: ViewportLineTruncationState::None,
        byte_range: ByteRange::new(0, text.len() as u64),
        utf16_range: Utf16Range {
            start: Utf16Position {
                line: 0,
                character: 0,
            },
            end: Utf16Position {
                line: 0,
                character: utf16_len,
            },
        },
        line_start_byte_offset: Some(0),
        logical_end_byte: Some(text.len() as u64),
        line_start_utf16_offset: Some(0),
    };
    editor_galley_for_geometry(ctx, &line, width)
}

fn shape_job(
    ctx: &Context,
    text: &str,
    width: f32,
    leading_space: f32,
    round_output_to_gui: bool,
    pixels_per_point: f32,
) -> std::sync::Arc<Galley> {
    let mut job = egui::text::LayoutJob::simple(
        text.to_owned(),
        egui::FontId::monospace(14.0),
        egui::Color32::WHITE,
        width,
    );
    job.sections[0].leading_space = leading_space;
    job.round_output_to_gui = round_output_to_gui;

    let mut galley = None;
    let mut raw_input = egui::RawInput::default();
    raw_input.viewports.insert(
        raw_input.viewport_id,
        egui::ViewportInfo {
            native_pixels_per_point: Some(pixels_per_point),
            ..Default::default()
        },
    );
    let _ = ctx.run_ui(raw_input, |ui| {
        galley = Some(ui.ctx().fonts_mut(|fonts| fonts.layout_job(job.clone())));
    });
    galley.expect("manual layout job should produce a galley")
}

#[derive(Clone, Debug, PartialEq)]
struct RowSignature {
    text: String,
    x: Vec<i32>,
    width: i32,
}

fn signatures(galley: &Galley) -> Vec<RowSignature> {
    galley
        .rows
        .iter()
        .map(|row| RowSignature {
            text: row.text().to_owned(),
            // Pixel rounding is intentional here: this experiment checks the
            // renderer's observable row geometry rather than private floats.
            x: row
                .glyphs
                .iter()
                .map(|glyph| (glyph.pos.x * 1000.0).round() as i32)
                .collect(),
            width: (row.size.x * 1000.0).round() as i32,
        })
        .collect()
}

fn row_texts(galley: &Galley) -> Vec<String> {
    galley
        .rows
        .iter()
        .map(|row| row.text().to_owned())
        .collect()
}

fn char_boundary(text: &str, chars: usize) -> usize {
    text.char_indices()
        .nth(chars)
        .map_or(text.len(), |(byte, _)| byte)
}

fn checkpoint_rows(ctx: &Context, text: &str, width: f32, split_chars: usize) -> Vec<RowSignature> {
    let split = char_boundary(text, split_chars);
    let first = shape(ctx, &text[..split], width);

    // Only completed rows are committed. The final row is provisional because
    // the next chunk may supply a whitespace/word-break candidate.
    let committed = first.rows.len().saturating_sub(1);
    let restart_chars: usize = first.rows[..committed]
        .iter()
        .map(|row| row.char_count_including_newline())
        .sum();
    let restart = char_boundary(text, restart_chars);
    let resumed = shape(ctx, &text[restart..], width);

    let mut output = signatures(&first)[..committed].to_vec();
    output.extend(signatures(&resumed));
    output
}

#[test]
fn restarting_at_provisional_row_is_known_pixel_phase_failure() {
    with_ui(|ui| {
        let ctx = ui.ctx().clone();
        let text = "alpha beta gamma delta epsilon zeta eta theta";
        let width = 92.0;
        let split = 13;
        let reference = shape(&ctx, text, width);
        let first = shape(&ctx, &text[..char_boundary(text, split)], width);
        let committed = first.rows.len().saturating_sub(1);
        let restart_chars: usize = first.rows[..committed]
            .iter()
            .map(|row| row.char_count_including_newline())
            .sum();
        let restart = char_boundary(text, restart_chars);
        let resumed = shape(&ctx, &text[restart..], width);

        assert_eq!(row_texts(&reference), {
            let mut rows = row_texts(&first)[..committed].to_vec();
            rows.extend(row_texts(&resumed));
            rows
        });
        let mut checkpointed = signatures(&first)[..committed].to_vec();
        checkpointed.extend(signatures(&resumed));
        assert_ne!(
            checkpointed,
            signatures(&reference),
            "the zero-pen restart must remain a visible counterexample"
        );
    });
}

fn phase_carried_rows(
    ctx: &Context,
    text: &str,
    width: f32,
    split_chars: usize,
    pixels_per_point: f32,
) -> Vec<RowSignature> {
    let split = char_boundary(text, split_chars);
    let first = shape_job(ctx, &text[..split], width, 0.0, true, pixels_per_point);
    let committed = first.rows.len().saturating_sub(1);
    let restart_chars: usize = first.rows[..committed]
        .iter()
        .map(|row| row.char_count_including_newline())
        .sum();
    let restart = char_boundary(text, restart_chars);

    // The metric pass deliberately disables GUI rounding so its intrinsic
    // width retains the physical pen phase needed by the resumed section.
    let prefix_metrics = shape_job(
        ctx,
        &text[..restart],
        f32::INFINITY,
        0.0,
        false,
        pixels_per_point,
    );
    let resumed = shape_job(
        ctx,
        &text[restart..],
        width,
        prefix_metrics.intrinsic_size().x,
        true,
        pixels_per_point,
    );

    let mut output = signatures(&first)[..committed].to_vec();
    output.extend(signatures(&resumed));
    output
}

#[test]
fn phase_carried_leading_space_inserts_one_empty_row_counterexample() {
    with_ui(|ui| {
        let ctx = ui.ctx().clone();
        let text = "alpha beta gamma delta epsilon zeta eta theta";
        for (pixels_per_point, width, split) in [(1.0, 92.0, 13), (1.5, 92.0, 13), (1.5, 87.0, 17)]
        {
            let reference = signatures(&shape_job(&ctx, text, width, 0.0, true, pixels_per_point));
            let first = shape_job(
                &ctx,
                &text[..char_boundary(text, split)],
                width,
                0.0,
                true,
                pixels_per_point,
            );
            let committed = first.rows.len().saturating_sub(1);
            let carried = phase_carried_rows(&ctx, text, width, split, pixels_per_point);

            // Exact observed relationship: the phase-carry continuation emits
            // one empty row at the restart boundary, while every baseline row
            // retains its text and rounded glyph geometry after the insertion.
            assert_eq!(carried.len(), reference.len() + 1);
            assert_eq!(&carried[..committed], &reference[..committed]);
            assert_eq!(
                carried[committed],
                RowSignature {
                    text: String::new(),
                    x: Vec::new(),
                    width: 0,
                },
                "vendor phase-carry inserted row at width {width}, split {split}, ppp {pixels_per_point}"
            );
            assert!(!reference[committed].text.is_empty());
            assert!(!reference[committed].x.is_empty());
            assert!(reference[committed].width > 0);
            assert_eq!(&carried[committed + 1..], &reference[committed..]);
        }
    });
}

#[test]
fn phase_carried_prefix_metrics_keeps_kerning_boundary_explicit() {
    with_ui(|ui| {
        let ctx = ui.ctx().clone();
        // At this width the committed row ends in `A` and the resumed row
        // starts in `V`; the resumed LayoutJob has no previous glyph id.
        let text = "AAAAAVAAAAAV";
        let width = 44.0;
        let split = 7;
        let pixels_per_point = 1.5;
        let reference = signatures(&shape_job(&ctx, text, width, 0.0, true, pixels_per_point));
        let carried = phase_carried_rows(&ctx, text, width, split, pixels_per_point);

        // The pen phase is carried, but the public LayoutJob seam still starts
        // with no previous glyph id. Keep this exact comparison as evidence of
        // the remaining kerning-state boundary rather than relaxing it.
        assert_ne!(carried, reference);
    });
}

#[test]
fn checkpoint_is_bounded_to_the_provisional_row_not_a_full_line_claim() {
    with_ui(|ui| {
        let ctx = ui.ctx().clone();
        let text = "one two three four five six seven eight nine ten";
        let width = 92.0;
        let first = shape(&ctx, &text[..char_boundary(text, 12)], width);
        assert!(!first.rows.is_empty());
        // This test documents the seam's bounded contract: the last row is
        // deliberately re-shaped, so it is never treated as a stable prefix.
        let committed = first.rows.len().saturating_sub(1);
        assert!(committed < first.rows.len());
        let checkpointed = checkpoint_rows(&ctx, text, width, 12);
        assert_eq!(checkpointed, signatures(&shape(&ctx, text, width)));
    });
}
