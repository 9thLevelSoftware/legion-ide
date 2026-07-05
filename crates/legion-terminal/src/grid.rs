//! Terminal grid projection helpers for renderer-owned terminal panels.
//!
//! The terminal runtime owns process lifecycle and bounded output capture. This
//! module converts the protocol projection into renderer-friendly rows without
//! introducing editor/app authority or retaining raw, unredacted terminal data.

use legion_protocol::{
    EventSequence, RedactionHint, TerminalOutputRowProjection, TerminalPanelProjection,
    TerminalScrollbackProjection,
};

/// Selection target for renderer copy operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminalGridSelection {
    /// Copy one projected output row by event sequence.
    Row(EventSequence),
    /// Copy all currently visible projected rows.
    AllVisible,
}

/// Renderer-friendly terminal grid state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalGrid {
    /// Bounded rows rendered by the terminal panel.
    pub rows: Vec<TerminalGridRow>,
    /// Scrollback metadata from the source projection.
    pub scrollback: TerminalScrollbackProjection,
}

/// Renderer-friendly terminal output row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerminalGridRow {
    /// Output sequence from the protocol projection.
    pub sequence: EventSequence,
    /// Fixed-width display label for sequence.
    pub sequence_label: String,
    /// Human stream label (`stdout` or `stderr`).
    pub stream_label: String,
    /// Bounded redacted payload to render/copy.
    pub payload: String,
    /// Non-authoritative renderer badges.
    pub badges: Vec<String>,
}

impl TerminalGrid {
    /// Build a renderer grid from the bounded terminal panel projection.
    ///
    /// `max_rows` is a renderer budget only; scrollback counts remain copied
    /// from the protocol projection so omitted/truncated state is still visible.
    pub fn from_projection(projection: &TerminalPanelProjection, max_rows: usize) -> Self {
        let rows = projection
            .output_rows
            .iter()
            .take(max_rows)
            .map(TerminalGridRow::from_projection)
            .collect();
        Self {
            rows,
            scrollback: projection.scrollback.clone(),
        }
    }

    /// Returns copy text for a selection, using already-redacted visible payloads.
    pub fn copy_selection(&self, selection: TerminalGridSelection) -> Option<String> {
        match selection {
            TerminalGridSelection::Row(sequence) => self
                .rows
                .iter()
                .find(|row| row.sequence == sequence)
                .map(|row| row.payload.clone()),
            TerminalGridSelection::AllVisible => {
                if self.rows.is_empty() {
                    None
                } else {
                    Some(
                        self.rows
                            .iter()
                            .map(|row| row.payload.as_str())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    )
                }
            }
        }
    }
}

impl TerminalGridRow {
    /// Converts a protocol output row to a renderer grid row.
    pub fn from_projection(row: &TerminalOutputRowProjection) -> Self {
        Self {
            sequence: row.sequence,
            sequence_label: format!("{:>4}", row.sequence.0),
            stream_label: if row.is_stderr { "stderr" } else { "stdout" }.to_string(),
            payload: row.redacted_payload.clone(),
            badges: terminal_output_row_badges(row),
        }
    }
}

/// Builds non-authoritative display badges for a terminal row.
pub fn terminal_output_row_badges(row: &TerminalOutputRowProjection) -> Vec<String> {
    let mut badges = Vec::new();
    if row.truncated {
        badges.push("truncated".to_string());
    }
    if row.redaction != RedactionHint::None {
        badges.push(format!("redacted:{:?}", row.redaction));
    }
    if row.byte_count > row.redacted_payload.len() as u64 {
        badges.push(format!("{} bytes", row.byte_count));
    }
    badges
}
