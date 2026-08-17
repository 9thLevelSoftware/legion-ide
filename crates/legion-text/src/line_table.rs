//! Per-line measurements, stored as a shared base plus a deferred-shift overlay.
//!
//! A keystroke that introduces no line break changes only two things about the line
//! table: the edited line grows or shrinks, and every line below it slides by the same
//! byte delta. Applying that eagerly means writing to every element, which forces a fresh
//! `Vec` per edit because [`crate::LineIndex`] is shared behind an `Arc` by every snapshot
//! taken since the last edit. At 100MB — roughly 1.7M lines of 48 bytes — that copy is
//! about 82MB per keystroke, and it was the whole of the measured typing cost.
//!
//! So the shift is recorded instead of applied. [`LineTable`] keeps the scanned metrics in
//! an `Arc` that is never mutated, plus a short list of pending shifts; a new table per
//! edit clones only that short list. Reads fold the pending shifts over the base metric on
//! the way out, which is why lookups return a [`LineMetric`] by value rather than by
//! reference.
//!
//! The overlay is bounded: once it reaches [`COMPACTION_THRESHOLD`] the shifts are folded
//! into a fresh base, paying the O(lines) copy once per that many keystrokes rather than
//! on every one. That makes the cost amortized, and the compacting keystroke is the worst
//! case — see the evidence note for both figures measured against budget.
//!
//! Both folds — the per-line one in [`LineTable::metric`] and the whole-table one in
//! [`LineTable::materialize`] — sum the pending deltas and apply them once, rather than
//! applying each pending shift in turn. That is what keeps compaction to a single pass
//! over the metrics; an earlier revision applied the shifts one at a time and cost 97ms
//! at a million lines, because it walked the whole table once per pending shift.
//!
//! The base metrics only ever come from `scan_line_metrics_from_byte`, so this module
//! cannot change which byte sequences end a line. It shifts offsets; it does not decide
//! line breaks. That separation is deliberate: line-break semantics are load-bearing for
//! every diagnostic, breakpoint and LSP position in the workspace.

use std::{cmp::Ordering, fmt, sync::Arc};

/// Pending shifts tolerated before the overlay is folded into a new base.
///
/// Reads walk the overlay, so this trades a small constant on every lookup against the
/// frequency of the O(lines) compaction. At 64 the per-keystroke amortized copy is a
/// sixty-fourth of the metrics vector.
const COMPACTION_THRESHOLD: usize = 64;

/// Byte and UTF-16 measurements for one logical line.
///
/// `content_end_byte` excludes the line ending; `end_byte` includes it. `byte_len` and
/// `utf16_len` are content lengths, so they also exclude the ending.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct LineMetric {
    pub(crate) start_byte: usize,
    pub(crate) content_end_byte: usize,
    pub(crate) end_byte: usize,
    pub(crate) byte_len: usize,
    pub(crate) utf16_len: usize,
    pub(crate) line_ending_bytes: usize,
}

impl LineMetric {
    /// Whether `offset` falls on this line. The last line owns its end offset so that a
    /// position at end-of-buffer resolves.
    pub(crate) fn contains_offset(&self, offset: usize, is_last_line: bool) -> bool {
        if is_last_line {
            self.start_byte <= offset && offset <= self.end_byte
        } else {
            self.start_byte <= offset && offset < self.end_byte
        }
    }

    /// Apply an already-summed shift to this line.
    ///
    /// `before` is the net byte movement caused by edits on earlier lines, which slides
    /// the whole line; `own_byte` and `own_utf16` are the net change from edits on this
    /// line, which alter its extent but not where it starts.
    fn shift(&mut self, before: isize, own_byte: isize, own_utf16: isize) {
        self.start_byte = shift_usize(self.start_byte, before);
        self.content_end_byte = shift_usize(self.content_end_byte, before + own_byte);
        self.end_byte = shift_usize(self.end_byte, before + own_byte);
        self.byte_len = shift_usize(self.byte_len, own_byte);
        self.utf16_len = shift_usize(self.utf16_len, own_utf16);
    }
}

/// One deferred edit: a line grew or shrank, and everything below it moved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LineShift {
    line: usize,
    byte_delta: isize,
    utf16_delta: isize,
}

/// The line table for one immutable revision of a buffer.
#[derive(Clone)]
pub(crate) struct LineTable {
    /// Scanned metrics, shared and never mutated once built.
    base: Arc<Vec<LineMetric>>,
    /// Edits applied since `base` was built, oldest first.
    overlay: Vec<LineShift>,
}

impl LineTable {
    /// Build a table directly from scanned metrics, with nothing pending.
    pub(crate) fn from_metrics(lines: Vec<LineMetric>) -> Self {
        Self {
            base: Arc::new(lines),
            overlay: Vec::new(),
        }
    }

    /// Number of logical lines. Simple edits never add or remove one, so this is the base
    /// length regardless of what is pending.
    pub(crate) fn len(&self) -> usize {
        self.base.len()
    }

    /// Resolve one line, folding any pending shifts over the shared base.
    pub(crate) fn metric(&self, line: usize) -> Option<LineMetric> {
        let mut metric = self.base.get(line)?.clone();
        if self.overlay.is_empty() {
            return Some(metric);
        }

        let (before, own_byte, own_utf16) = self.summed_deltas_for(line);
        metric.shift(before, own_byte, own_utf16);
        Some(metric)
    }

    /// Net pending movement affecting `line`: from earlier lines, and from itself.
    fn summed_deltas_for(&self, line: usize) -> (isize, isize, isize) {
        let mut before = 0isize;
        let mut own_byte = 0isize;
        let mut own_utf16 = 0isize;
        for shift in &self.overlay {
            match shift.line.cmp(&line) {
                Ordering::Less => before += shift.byte_delta,
                Ordering::Equal => {
                    own_byte += shift.byte_delta;
                    own_utf16 += shift.utf16_delta;
                }
                Ordering::Greater => {}
            }
        }
        (before, own_byte, own_utf16)
    }

    /// Record an edit on `line` that moved `byte_delta` bytes and `utf16_delta` UTF-16
    /// units, returning the resulting table.
    ///
    /// Returns `None` when `line` is out of range, matching the previous behaviour of
    /// failing the fast path so the caller falls back to a full rebuild.
    pub(crate) fn with_simple_edit(
        &self,
        line: usize,
        byte_delta: isize,
        utf16_delta: isize,
    ) -> Option<Self> {
        if line >= self.base.len() {
            return None;
        }

        let shift = LineShift {
            line,
            byte_delta,
            utf16_delta,
        };

        // Fold rather than grow once the overlay has reached its bound, so lookups stay
        // cheap and memory does not track edit count.
        if self.overlay.len() + 1 >= COMPACTION_THRESHOLD {
            let mut folded = self.clone();
            folded.overlay.push(shift);
            return Some(Self::from_metrics(folded.materialize()));
        }

        let mut overlay = Vec::with_capacity(self.overlay.len() + 1);
        overlay.extend_from_slice(&self.overlay);
        overlay.push(shift);
        Some(Self {
            base: Arc::clone(&self.base),
            overlay,
        })
    }

    /// Produce a flat vector with every pending shift applied.
    ///
    /// One pass over the metrics regardless of how many shifts are pending: the shifts are
    /// ordered by line, then a running total is carried down the table. Memory traffic is
    /// therefore the size of the table, not the size of the table times the overlay.
    pub(crate) fn materialize(&self) -> Vec<LineMetric> {
        if self.overlay.is_empty() {
            return self.base.as_ref().clone();
        }

        let mut ordered = self.overlay.clone();
        ordered.sort_by_key(|shift| shift.line);

        // Built straight into a fresh vector rather than cloned and then rewritten, which
        // would touch the metrics twice for no gain. This is the whole cost of a
        // compacting keystroke, so the constant factor is worth the slightly longer loop.
        let mut lines = Vec::with_capacity(self.base.len());
        let mut before = 0isize;
        let mut cursor = 0usize;
        for (index, base) in self.base.iter().enumerate() {
            let mut own_byte = 0isize;
            let mut own_utf16 = 0isize;
            while cursor < ordered.len() && ordered[cursor].line == index {
                own_byte += ordered[cursor].byte_delta;
                own_utf16 += ordered[cursor].utf16_delta;
                cursor += 1;
            }

            let mut metric = base.clone();
            metric.shift(before, own_byte, own_utf16);
            lines.push(metric);
            before += own_byte;
        }
        lines
    }

    /// Resolve the first `upto` lines into a flat vector, for rebuild paths that keep a
    /// prefix and rescan the remainder.
    pub(crate) fn prefix(&self, upto: usize) -> Vec<LineMetric> {
        let mut lines = self.materialize();
        lines.truncate(upto.min(self.len()));
        lines
    }

    /// Find the line containing `offset`.
    pub(crate) fn index_for_offset(&self, offset: usize) -> Option<usize> {
        search_for_offset(self.len(), offset, |line| {
            self.metric(line).expect("line is within the table")
        })
    }
}

impl PartialEq for LineTable {
    /// Compares resolved lines, so two tables holding the same mapping are equal whether
    /// or not their shifts have been folded in yet.
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if Arc::ptr_eq(&self.base, &other.base) && self.overlay == other.overlay {
            return true;
        }
        self.materialize() == other.materialize()
    }
}

impl Eq for LineTable {}

impl fmt::Debug for LineTable {
    /// Summarized rather than exhaustive: a formatted table would otherwise print every
    /// line of the buffer.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LineTable")
            .field("lines", &self.len())
            .field("pending_shifts", &self.overlay.len())
            .finish()
    }
}

/// Find the line containing `offset` in a flat slice of metrics.
///
/// Used while building chunk descriptors, where the metrics have already been
/// materialized. Shares its algorithm with [`LineTable::index_for_offset`] so the two
/// cannot drift apart.
pub(crate) fn index_for_offset_in(lines: &[LineMetric], offset: usize) -> Option<usize> {
    search_for_offset(lines.len(), offset, |line| lines[line].clone())
}

/// The line lookup itself, over anything that can produce a metric by index.
///
/// This mirrors the `binary_search_by` that previously ran over the flat vector exactly,
/// including which candidate a run of equal-comparing lines settles on and the forward
/// walk that follows. Resolved line starts remain monotonically non-decreasing — a
/// deletion can never remove more than the edited line holds — so the search stays valid
/// over a table with pending shifts.
fn search_for_offset(
    len: usize,
    offset: usize,
    metric_at: impl Fn(usize) -> LineMetric,
) -> Option<usize> {
    if len == 0 {
        return None;
    }

    let mut left = 0usize;
    let mut right = len;
    let mut size = len;
    let mut found = None;

    while left < right {
        let mid = left + size / 2;
        let metric = metric_at(mid);
        if offset < metric.start_byte {
            right = mid;
        } else if offset > metric.end_byte {
            left = mid + 1;
        } else {
            found = Some(mid);
            break;
        }
        size = right - left;
    }

    match found {
        Some(mut index) => {
            while index + 1 < len && !metric_at(index).contains_offset(offset, index + 1 == len) {
                index += 1;
            }
            Some(index)
        }
        None if left > 0 => Some(left - 1),
        None => Some(0),
    }
}

/// Offset a `usize` by a signed delta, saturating rather than wrapping at either end.
pub(crate) fn shift_usize(base: usize, delta: isize) -> usize {
    if delta >= 0 {
        base.saturating_add(delta as usize)
    } else {
        base.saturating_sub(delta.unsigned_abs())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn metric(start: usize, len: usize, ending: usize) -> LineMetric {
        LineMetric {
            start_byte: start,
            content_end_byte: start + len,
            end_byte: start + len + ending,
            byte_len: len,
            utf16_len: len,
            line_ending_bytes: ending,
        }
    }

    fn table() -> LineTable {
        LineTable::from_metrics(vec![
            metric(0, 5, 1),
            metric(6, 4, 1),
            metric(11, 5, 1),
            metric(17, 0, 0),
        ])
    }

    /// Independent reference: apply one edit the direct way, writing through every line.
    ///
    /// This is the implementation the table replaced. Keeping it here as an oracle means
    /// the deferred and compacted paths are checked against something that shares none of
    /// their machinery.
    fn apply_directly(
        lines: &mut [LineMetric],
        line: usize,
        byte_delta: isize,
        utf16_delta: isize,
    ) {
        if let Some(edited) = lines.get_mut(line) {
            edited.content_end_byte = shift_usize(edited.content_end_byte, byte_delta);
            edited.end_byte = shift_usize(edited.end_byte, byte_delta);
            edited.byte_len = shift_usize(edited.byte_len, byte_delta);
            edited.utf16_len = shift_usize(edited.utf16_len, utf16_delta);
        }
        for metric in lines.iter_mut().skip(line + 1) {
            metric.start_byte = shift_usize(metric.start_byte, byte_delta);
            metric.content_end_byte = shift_usize(metric.content_end_byte, byte_delta);
            metric.end_byte = shift_usize(metric.end_byte, byte_delta);
        }
    }

    #[test]
    fn a_pending_shift_resolves_the_same_as_writing_through() {
        let deferred = table().with_simple_edit(1, 2, 2).expect("line in range");
        let mut direct = table().materialize();
        apply_directly(&mut direct, 1, 2, 2);

        assert_eq!(deferred.materialize(), direct);
        for line in 0..direct.len() {
            assert_eq!(deferred.metric(line), Some(direct[line].clone()));
        }
    }

    #[test]
    fn overlapping_shifts_on_several_lines_resolve_the_same_as_writing_through() {
        let edits = [
            (2usize, 3isize, 3isize),
            (0, -1, -1),
            (1, 4, 2),
            (2, -2, -2),
        ];
        let mut deferred = table();
        let mut direct = table().materialize();
        for (line, byte_delta, utf16_delta) in edits {
            deferred = deferred
                .with_simple_edit(line, byte_delta, utf16_delta)
                .expect("line in range");
            apply_directly(&mut direct, line, byte_delta, utf16_delta);
        }

        assert_eq!(deferred.materialize(), direct);
        for line in 0..direct.len() {
            assert_eq!(deferred.metric(line), Some(direct[line].clone()));
        }
    }

    #[test]
    fn compaction_preserves_the_mapping() {
        // Drive well past the threshold so several compactions happen.
        let mut deferred = table();
        let mut direct = table().materialize();
        for step in 0..(COMPACTION_THRESHOLD * 3) {
            let line = step % 3;
            deferred = deferred
                .with_simple_edit(line, 1, 1)
                .expect("line in range");
            apply_directly(&mut direct, line, 1, 1);
        }
        assert_eq!(deferred.materialize(), direct);
    }

    #[test]
    fn the_base_is_shared_rather_than_copied_between_edits() {
        let first = table();
        let second = first.with_simple_edit(0, 1, 1).expect("line in range");
        assert!(
            Arc::ptr_eq(&first.base, &second.base),
            "an edit below the compaction threshold must not rebuild the base"
        );
    }

    #[test]
    fn compaction_resets_the_overlay() {
        let mut table = table();
        for _ in 0..COMPACTION_THRESHOLD {
            table = table.with_simple_edit(0, 1, 1).expect("line in range");
        }
        assert!(
            table.overlay.len() < COMPACTION_THRESHOLD,
            "the overlay must be bounded by the compaction threshold"
        );
    }

    #[test]
    fn an_edit_beyond_the_last_line_declines_the_fast_path() {
        assert!(table().with_simple_edit(99, 1, 1).is_none());
    }

    #[test]
    fn lookups_match_a_linear_scan_over_the_resolved_table() {
        let deferred = table()
            .with_simple_edit(1, 3, 3)
            .and_then(|table| table.with_simple_edit(0, -1, -1))
            .expect("lines in range");
        let lines = deferred.materialize();

        for offset in 0..=lines.last().expect("non-empty").end_byte {
            let expected = lines
                .iter()
                .position(|line| line.contains_offset(offset, false))
                .or_else(|| {
                    lines
                        .last()
                        .filter(|line| line.contains_offset(offset, true))
                        .map(|_| lines.len() - 1)
                });
            if let Some(expected) = expected {
                assert_eq!(
                    deferred.index_for_offset(offset),
                    Some(expected),
                    "offset {offset}"
                );
            }
        }
    }
}
