//! Exact, bounded small-document accessibility for the custom code canvas.
//! Cached runs are a presentation projection, never editable text authority.

use egui::accesskit::{Node, Role};
use legion_ui::{
    ActiveBufferProjection, ActiveBufferProjectionState,
    ui::{
        EDITOR_ACCESSIBILITY_MAX_TEXT_BYTES, EditorAccessibilityCoverage,
        EditorAccessibilityProjection,
    },
};
use unicode_segmentation::UnicodeSegmentation;

/// Maximum selectable graphemes in one complete published document.
pub const MAX_SELECTABLE_UNITS: usize = 65_536;
/// Maximum text-run descendants in one complete published document.
pub const MAX_TEXT_RUNS: usize = 1_024;
/// Character-length metadata is stored as one u8 per selectable unit.
pub const MAX_CHARACTER_METADATA_BYTES: usize = 65_536;
/// Byte-boundary metadata needed to map a directed selection into cached runs.
const MAX_SELECTION_METADATA_BYTES: usize =
    (MAX_SELECTABLE_UNITS + MAX_TEXT_RUNS) * std::mem::size_of::<usize>();
const MAX_RUN_UNITS: usize = 255;

pub(crate) fn document_widget_id() -> egui::Id {
    egui::Id::new("legion_editor_document")
}

#[derive(Debug)]
struct TextRun {
    node: Node,
    byte_boundaries: Vec<usize>,
}

#[derive(Debug, Default)]
pub(super) struct EditorAccessibility {
    last_active_buffer: Option<legion_protocol::BufferId>,
    cached: Option<(
        EditorAccessibilityProjection,
        Result<Vec<TextRun>, &'static str>,
    )>,
}

impl EditorAccessibility {
    pub(super) fn publish(
        &mut self,
        ui: &mut egui::Ui,
        active: &ActiveBufferProjection,
        viewport_rect: egui::Rect,
    ) {
        if active.buffer_id.is_none() {
            self.cached = None;
            self.last_active_buffer = None;
            ui.memory_mut(|memory| memory.surrender_focus(document_widget_id()));
            return;
        }
        let activated = self.last_active_buffer != active.buffer_id;
        self.last_active_buffer = active.buffer_id;
        if activated
            && ui.is_enabled()
            && !ui.input(|input| input.pointer.any_pressed())
            && ui.memory(|memory| memory.focused().is_none())
        {
            // Initial/file-open activation can claim an unowned keyboard, but
            // never takes it from an already focused control or pointer gesture.
            ui.memory_mut(|memory| memory.request_focus(document_widget_id()));
        }
        // Register focus interest only to retain keyboard ownership already
        // acquired by a real canvas activation. An unsolicited AT request must
        // not transfer focus away from another control.
        let owns_keyboard = ui.memory(|memory| memory.has_focus(document_widget_id()));
        let mut document_ui = ui.new_child(
            egui::UiBuilder::new()
                .id(document_widget_id())
                .sense(if owns_keyboard {
                    egui::Sense::focusable_noninteractive()
                } else {
                    egui::Sense::hover()
                })
                .max_rect(viewport_rect),
        );
        document_ui.set_min_size(viewport_rect.size());
        if owns_keyboard {
            // egui computes spatial navigation before this frame's filter is
            // installed. On the first arrow after activation, cancel that one
            // pending spatial move; the app still consumes the original key.
            let editor_arrow = ui.input(|input| {
                !input.modifiers.any()
                    && [
                        egui::Key::ArrowLeft,
                        egui::Key::ArrowRight,
                        egui::Key::ArrowUp,
                        egui::Key::ArrowDown,
                    ]
                    .iter()
                    .any(|key| input.key_pressed(*key))
            });
            ui.memory_mut(|memory| {
                if editor_arrow {
                    memory.move_focus(egui::FocusDirection::None);
                }
                memory.set_focus_lock_filter(
                    document_widget_id(),
                    egui::EventFilter {
                        horizontal_arrows: true,
                        vertical_arrows: true,
                        ..Default::default()
                    },
                );
            });
        }
        let id = document_ui.unique_id();
        // Do not build or retain text when the platform has not enabled a11y.
        if ui.ctx().accesskit_node_builder(id, |_| ()).is_none() {
            self.cached = None;
            return;
        }
        let text = eligible_text(active);
        let runs = match text {
            Ok((metadata, text)) => {
                // Selection changes do not invalidate immutable text segmentation.
                let mut text_identity = metadata;
                text_identity.primary_selection = None;
                if self.cached.as_ref().map(|cached| cached.0) != Some(text_identity) {
                    self.cached = Some((text_identity, build_runs(text)));
                } else if self.cached.as_ref().is_some_and(|(_, cached)| {
                    cached.as_ref().is_ok_and(|runs| {
                        !runs
                            .iter()
                            .flat_map(|run| run.node.value().unwrap_or_default().bytes())
                            .eq(text.bytes())
                    })
                }) {
                    // An unchanged authoritative identity cannot name different
                    // source bytes. Keep it unavailable until a fresh identity;
                    // compare cached payloads without another full text copy.
                    self.cached.as_mut().expect("cache populated").1 =
                        Err("contradictory preview under unchanged identity");
                }
                &self.cached.as_ref().expect("cache populated").1
            }
            Err(reason) => {
                publish_unavailable(ui.ctx(), id, viewport_rect, reason);
                return;
            }
        };
        let runs = match runs {
            Ok(runs) => runs,
            Err(reason) => {
                publish_unavailable(ui.ctx(), id, viewport_rect, reason);
                return;
            }
        };
        let run_ids: Vec<_> = (0..runs.len())
            .map(|index| {
                document_ui
                    .new_child(egui::UiBuilder::new().id_salt(("editor_text_run", index)))
                    .unique_id()
            })
            .collect();
        let selection = active
            .accessibility
            .and_then(|metadata| metadata.primary_selection)
            .and_then(|selection| {
                Some(egui::accesskit::TextSelection {
                    anchor: position(runs, &run_ids, selection.anchor_byte)?,
                    focus: position(runs, &run_ids, selection.focus_byte)?,
                })
            });
        ui.ctx().accesskit_node_builder(id, |node| {
            node.set_role(Role::MultilineTextInput);
            node.set_label("Editor document");
            node.set_description("Complete bounded small-buffer text");
            node.set_bounds(bounds(viewport_rect));
            node.remove_action(egui::accesskit::Action::Focus);
            if !active.accessibility.expect("validated metadata").editable {
                node.set_read_only();
            }
            if let Some(selection) = selection {
                node.set_text_selection(selection);
            }
            // No Focus or SetTextSelection actions are advertised. Native input
            // remains on the existing canvas path, not a new egui text widget.
        });
        for (index, (run_id, run)) in run_ids.iter().zip(runs).enumerate() {
            ui.ctx().accesskit_node_builder(*run_id, |node| {
                // egui emits fresh nodes each pass. Only this required tree copy
                // is made; segmentation and character metadata are cached.
                *node = run.node.clone();
                // The 255-unit storage boundary is not a logical line break.
                // Link fragments with this frame's IDs, but never cross LF/CRLF.
                if index > 0
                    && !runs[index - 1]
                        .node
                        .value()
                        .unwrap_or_default()
                        .ends_with('\n')
                {
                    node.set_previous_on_line(run_ids[index - 1].accesskit_id());
                }
                if index + 1 < runs.len() && !run.node.value().unwrap_or_default().ends_with('\n') {
                    node.set_next_on_line(run_ids[index + 1].accesskit_id());
                }
            });
        }
    }
}

fn eligible_text(
    active: &ActiveBufferProjection,
) -> Result<(EditorAccessibilityProjection, &str), &'static str> {
    let metadata = active
        .accessibility
        .ok_or("document metadata unavailable")?;
    if active.degraded || active.state == ActiveBufferProjectionState::Degraded {
        return Err("degraded buffer");
    }
    match metadata.coverage {
        EditorAccessibilityCoverage::CompleteSmallBuffer => {}
        EditorAccessibilityCoverage::Degraded => return Err("degraded buffer"),
        EditorAccessibilityCoverage::PreviewUnavailable => return Err("exact preview unavailable"),
        EditorAccessibilityCoverage::TextBudgetExceeded => return Err("text byte budget exceeded"),
    }
    let viewport = active
        .viewport
        .as_ref()
        .ok_or("snapshot identity unavailable")?;
    if active.buffer_id != Some(metadata.buffer_id)
        || viewport.buffer_id != metadata.buffer_id
        || viewport.snapshot_id != metadata.snapshot_id
        || viewport.buffer_version != metadata.buffer_version
        || viewport.mode != legion_protocol::ViewportProjectionMode::Normal
    {
        return Err("snapshot identity or mode mismatch");
    }
    let text = active
        .small_buffer_text()
        .ok_or("exact preview unavailable")?;
    if text.len() > EDITOR_ACCESSIBILITY_MAX_TEXT_BYTES {
        return Err("text byte budget exceeded");
    }
    if text.len() != metadata.byte_len {
        return Err("preview byte length mismatch");
    }
    Ok((metadata, text))
}

fn build_runs(text: &str) -> Result<Vec<TextRun>, &'static str> {
    let mut runs = Vec::new();
    let mut units = 0;
    let mut value = String::new();
    let mut lengths = Vec::new();
    let mut byte = 0;
    let mut boundaries = vec![0];
    for grapheme in text.graphemes(true) {
        units += 1;
        if units > MAX_SELECTABLE_UNITS || units > MAX_CHARACTER_METADATA_BYTES {
            return Err("character metadata budget exceeded");
        }
        let length = u8::try_from(grapheme.len())
            .map_err(|_| "selectable unit exceeds character-length representation")?;
        value.push_str(grapheme);
        lengths.push(length);
        byte += grapheme.len();
        boundaries.push(byte);
        if lengths.len() == MAX_RUN_UNITS || grapheme.ends_with('\n') {
            push_run(&mut runs, &mut value, &mut lengths, &mut boundaries)?;
        }
    }
    // An empty run gives empty documents a real document range. A trailing
    // newline gets a final empty paragraph and a valid end-of-document caret.
    if !value.is_empty() || runs.is_empty() || text.ends_with('\n') {
        push_run(&mut runs, &mut value, &mut lengths, &mut boundaries)?;
    }
    Ok(runs)
}

fn push_run(
    runs: &mut Vec<TextRun>,
    value: &mut String,
    lengths: &mut Vec<u8>,
    boundaries: &mut Vec<usize>,
) -> Result<(), &'static str> {
    if runs.len() == MAX_TEXT_RUNS {
        return Err("text run node budget exceeded");
    }
    let mut node = Node::new(Role::TextRun);
    node.set_value(std::mem::take(value));
    node.set_character_lengths(std::mem::take(lengths));
    let end_byte = *boundaries.last().expect("at least the run start");
    runs.push(TextRun {
        node,
        byte_boundaries: std::mem::replace(boundaries, vec![end_byte]),
    });
    let metadata_bytes = runs
        .iter()
        .map(|run| run.byte_boundaries.len())
        .sum::<usize>()
        * std::mem::size_of::<usize>();
    if metadata_bytes > MAX_SELECTION_METADATA_BYTES {
        return Err("selection metadata budget exceeded");
    }
    Ok(())
}

fn position(
    runs: &[TextRun],
    ids: &[egui::Id],
    byte: usize,
) -> Option<egui::accesskit::TextPosition> {
    for (index, run) in runs.iter().enumerate() {
        let end = *run.byte_boundaries.last()?;
        if byte == end && index + 1 < runs.len() && run.node.value()?.ends_with('\n') {
            continue;
        }
        if let Ok(character_index) = run.byte_boundaries.binary_search(&byte) {
            return Some(egui::accesskit::TextPosition {
                node: ids[index].accesskit_id(),
                character_index,
            });
        }
    }
    None
}

fn bounds(rect: egui::Rect) -> egui::accesskit::Rect {
    egui::accesskit::Rect {
        x0: rect.left().into(),
        y0: rect.top().into(),
        x1: rect.right().into(),
        y1: rect.bottom().into(),
    }
}

fn publish_unavailable(ctx: &egui::Context, id: egui::Id, rect: egui::Rect, reason: &str) {
    ctx.accesskit_node_builder(id, |node| {
        node.set_role(Role::Group);
        node.set_label("Editor document");
        node.set_description(format!("Complete text unavailable: {reason}"));
        node.set_bounds(bounds(rect));
        node.remove_action(egui::accesskit::Action::Focus);
    });
}
