//! Line-level diff computation engine for multi-file proposal review surfaces.
//!
//! Implements an LCS-based diff algorithm that produces grouped [`DiffHunk`]
//! descriptors suitable for feeding into [`ProposalDiffSurfaceSectionProjection`].
//!
//! # Algorithm
//!
//! 1. Split each text into lines with [`str::lines`].
//! 2. Build the longest-common-subsequence DP table (`O(m×n)` space/time).
//! 3. Trace back iteratively to produce a flat list of `Keep`, `Add`, `Remove` ops.
//! 4. Group adjacent changes into `DiffHunk` values, merging when context windows overlap.

use legion_protocol::{
    CanonicalPath, FileId, ProposalDiffChunkDescriptor, ProposalDiffSurfaceAnchorProjection,
    ProposalDiffSurfaceSectionProjection, ProposalId,
};

// ─── Public API ──────────────────────────────────────────────────────────────

/// One changed region in a line-level diff.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiffHunk {
    /// 0-based start line in the old text (inclusive).
    pub old_start: usize,
    /// Number of lines from the old text represented in this hunk (including context).
    pub old_count: usize,
    /// 0-based start line in the new text (inclusive).
    pub new_start: usize,
    /// Number of lines from the new text represented in this hunk (including context).
    pub new_count: usize,
    /// Ordered diff lines for this hunk.
    pub lines: Vec<DiffLine>,
}

impl DiffHunk {
    /// Number of lines added in this hunk (does not include context lines).
    pub fn added_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Added(_)))
            .count()
    }

    /// Number of lines removed in this hunk (does not include context lines).
    pub fn removed_count(&self) -> usize {
        self.lines
            .iter()
            .filter(|l| matches!(l, DiffLine::Removed(_)))
            .count()
    }

    /// Convert this hunk to a [`ProposalDiffChunkDescriptor`].
    pub fn to_chunk_descriptor(
        &self,
        chunk_id: impl Into<String>,
        target_id: Option<String>,
    ) -> ProposalDiffChunkDescriptor {
        let inserted = self.added_count() as u32;
        let deleted = self.removed_count() as u32;
        ProposalDiffChunkDescriptor {
            chunk_id: chunk_id.into(),
            target_id,
            byte_range: None,
            changed_line_count: inserted + deleted,
            inserted_line_count: inserted,
            deleted_line_count: deleted,
            content_hash: None,
        }
    }

    /// Convert this single hunk into a [`ProposalDiffSurfaceSectionProjection`].
    ///
    /// Use [`diff_hunks_to_section_projection`] when converting multiple hunks
    /// for one file into a single section.
    pub fn to_section_projection(
        &self,
        proposal_id: ProposalId,
        section_index: usize,
        file_id: Option<FileId>,
        path: Option<CanonicalPath>,
    ) -> ProposalDiffSurfaceSectionProjection {
        diff_hunks_to_section_projection(
            std::slice::from_ref(self),
            proposal_id,
            section_index,
            file_id,
            path,
            None,
        )
    }
}

/// One line in a diff hunk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiffLine {
    /// Unchanged context line (present in both old and new).
    Context(String),
    /// Line added in the new version.
    Added(String),
    /// Line removed from the old version.
    Removed(String),
}

/// Compute line-level diff hunks between two text versions.
///
/// Returns hunks in document order (earliest changed region first).  Each hunk
/// includes up to [`CONTEXT_LINES`] surrounding context lines.  When two
/// adjacent clusters of changes would produce overlapping context windows the
/// hunks are merged into one.
///
/// If `old_text == new_text` the result is an empty `Vec`.
pub fn compute_line_diff(old_text: &str, new_text: &str) -> Vec<DiffHunk> {
    let old_lines: Vec<&str> = old_text.lines().collect();
    let new_lines: Vec<&str> = new_text.lines().collect();
    let ops = lcs_ops(&old_lines, &new_lines);
    group_into_hunks(&ops, &old_lines, &new_lines)
}

/// Convert a slice of [`DiffHunk`]s into a single
/// [`ProposalDiffSurfaceSectionProjection`].
///
/// Each hunk becomes one [`ProposalDiffChunkDescriptor`] inside the section.
pub fn diff_hunks_to_section_projection(
    hunks: &[DiffHunk],
    proposal_id: ProposalId,
    section_index: usize,
    file_id: Option<FileId>,
    path: Option<CanonicalPath>,
    target_id: Option<String>,
) -> ProposalDiffSurfaceSectionProjection {
    let section_id = format!("proposal:{}:diff-section:{}", proposal_id.0, section_index);
    let title = path
        .as_ref()
        .and_then(|p| std::path::Path::new(&p.0).file_name())
        .and_then(|n| n.to_str())
        .filter(|n| !n.is_empty())
        .map(|n| n.to_string())
        .or_else(|| target_id.clone())
        .unwrap_or_else(|| format!("section-{}", section_index));

    let chunks: Vec<ProposalDiffChunkDescriptor> = hunks
        .iter()
        .enumerate()
        .map(|(i, hunk)| {
            hunk.to_chunk_descriptor(
                format!("{}:chunk:{}", section_id, i),
                target_id.clone(),
            )
        })
        .collect();

    let anchor = ProposalDiffSurfaceAnchorProjection {
        target_id: target_id.clone(),
        workspace_id: None,
        file_id,
        buffer_id: None,
        path: path.clone(),
        byte_ranges: Vec::new(),
        line_ranges: Vec::new(),
        snapshot_id: None,
        redaction_hints: Vec::new(),
    };

    ProposalDiffSurfaceSectionProjection {
        section_id,
        proposal_id,
        target_id,
        title,
        anchor,
        chunks,
        redaction_hints: Vec::new(),
        schema_version: 1,
    }
}

// ─── Internal implementation ─────────────────────────────────────────────────

/// Number of unchanged context lines to include on each side of a changed region.
const CONTEXT_LINES: usize = 3;

/// Raw diff operation produced by the LCS trace-back.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Keep,
    Add,
    Remove,
}

/// Compute the sequence of [`Op`]s that transforms `old` into `new`.
///
/// Uses standard O(m×n) LCS dynamic programming with iterative back-trace to
/// avoid stack-overflow on large files.
fn lcs_ops(old: &[&str], new: &[&str]) -> Vec<Op> {
    let m = old.len();
    let n = new.len();

    // DP table: dp[i][j] = LCS length for old[..i] and new[..j].
    // Stored as a flat Vec to avoid Vec-of-Vec allocation overhead.
    let row_width = n + 1;
    let mut dp = vec![0u32; (m + 1) * row_width];

    for i in 1..=m {
        for j in 1..=n {
            dp[i * row_width + j] = if old[i - 1] == new[j - 1] {
                dp[(i - 1) * row_width + (j - 1)] + 1
            } else {
                dp[(i - 1) * row_width + j].max(dp[i * row_width + (j - 1)])
            };
        }
    }

    // Iterative trace-back: walk from (m, n) to (0, 0) recording ops in reverse.
    let mut ops = Vec::with_capacity(m + n);
    let mut i = m;
    let mut j = n;
    while i > 0 || j > 0 {
        if i > 0 && j > 0 && old[i - 1] == new[j - 1] {
            ops.push(Op::Keep);
            i -= 1;
            j -= 1;
        } else if j > 0 && (i == 0 || dp[i * row_width + (j - 1)] >= dp[(i - 1) * row_width + j])
        {
            ops.push(Op::Add);
            j -= 1;
        } else {
            ops.push(Op::Remove);
            i -= 1;
        }
    }
    ops.reverse();
    ops
}

/// Group the flat op sequence into context-bounded [`DiffHunk`]s.
fn group_into_hunks(ops: &[Op], old: &[&str], new: &[&str]) -> Vec<DiffHunk> {
    if ops.is_empty() {
        return Vec::new();
    }

    // Assign old/new line indices to each op position.
    struct Entry {
        op: Op,
        old_idx: Option<usize>,
        new_idx: Option<usize>,
    }

    let mut entries: Vec<Entry> = Vec::with_capacity(ops.len());
    let (mut oi, mut ni) = (0usize, 0usize);
    for &op in ops {
        entries.push(Entry {
            op,
            old_idx: if op != Op::Add { Some(oi) } else { None },
            new_idx: if op != Op::Remove { Some(ni) } else { None },
        });
        if op != Op::Add {
            oi += 1;
        }
        if op != Op::Remove {
            ni += 1;
        }
    }

    // Positions in `entries` that have a change (Add or Remove).
    let changed_positions: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter(|(_, e)| e.op != Op::Keep)
        .map(|(i, _)| i)
        .collect();

    if changed_positions.is_empty() {
        return Vec::new();
    }

    // Merge nearby changed positions into inclusive entry ranges.
    let last_idx = entries.len().saturating_sub(1);
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    let mut range_start = changed_positions[0].saturating_sub(CONTEXT_LINES);
    let mut range_end = (changed_positions[0] + CONTEXT_LINES).min(last_idx);

    for &pos in &changed_positions[1..] {
        let new_start = pos.saturating_sub(CONTEXT_LINES);
        let new_end = (pos + CONTEXT_LINES).min(last_idx);
        if new_start <= range_end + 1 {
            // Adjacent or overlapping context windows: extend the current range.
            range_end = new_end;
        } else {
            ranges.push((range_start, range_end));
            range_start = new_start;
            range_end = new_end;
        }
    }
    ranges.push((range_start, range_end));

    // Build one DiffHunk per range.
    ranges
        .into_iter()
        .map(|(start, end)| {
            let slice = &entries[start..=end];
            let old_start = slice.iter().find_map(|e| e.old_idx).unwrap_or(0);
            let old_count = slice.iter().filter(|e| e.old_idx.is_some()).count();
            let new_start = slice.iter().find_map(|e| e.new_idx).unwrap_or(0);
            let new_count = slice.iter().filter(|e| e.new_idx.is_some()).count();
            let lines = slice
                .iter()
                .map(|e| match e.op {
                    Op::Keep => DiffLine::Context(old[e.old_idx.unwrap()].to_string()),
                    Op::Add => DiffLine::Added(new[e.new_idx.unwrap()].to_string()),
                    Op::Remove => DiffLine::Removed(old[e.old_idx.unwrap()].to_string()),
                })
                .collect();
            DiffHunk {
                old_start,
                old_count,
                new_start,
                new_count,
                lines,
            }
        })
        .collect()
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use legion_protocol::ProposalId;

    fn added_lines(hunk: &DiffHunk) -> Vec<&str> {
        hunk.lines
            .iter()
            .filter_map(|l| {
                if let DiffLine::Added(s) = l {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    fn removed_lines(hunk: &DiffHunk) -> Vec<&str> {
        hunk.lines
            .iter()
            .filter_map(|l| {
                if let DiffLine::Removed(s) = l {
                    Some(s.as_str())
                } else {
                    None
                }
            })
            .collect()
    }

    // T1: No change — empty hunk list.
    #[test]
    fn no_change_produces_no_hunks() {
        let text = "line one\nline two\nline three\n";
        let hunks = compute_line_diff(text, text);
        assert!(
            hunks.is_empty(),
            "identical texts should produce zero hunks"
        );
    }

    // T2: Single-line change — exactly one hunk with one removed + one added line.
    #[test]
    fn single_line_change_produces_one_hunk() {
        let old = "alpha\nbeta\ngamma\n";
        let new = "alpha\nBETA\ngamma\n";
        let hunks = compute_line_diff(old, new);
        assert_eq!(hunks.len(), 1, "single changed line → one hunk");
        let h = &hunks[0];
        assert_eq!(removed_lines(h), vec!["beta"]);
        assert_eq!(added_lines(h), vec!["BETA"]);
    }

    // T3: Multi-line insert — one hunk with multiple added lines.
    #[test]
    fn multi_line_insert_produces_one_hunk() {
        let old = "before\nafter\n";
        let new = "before\ninsert1\ninsert2\ninsert3\nafter\n";
        let hunks = compute_line_diff(old, new);
        assert_eq!(hunks.len(), 1, "contiguous inserts → one hunk");
        let h = &hunks[0];
        assert_eq!(h.added_count(), 3);
        assert_eq!(h.removed_count(), 0);
        assert_eq!(added_lines(h), vec!["insert1", "insert2", "insert3"]);
    }

    // T4: Multi-line delete — one hunk with multiple removed lines.
    #[test]
    fn multi_line_delete_produces_one_hunk() {
        let old = "keep\ndel1\ndel2\ndel3\nkeep2\n";
        let new = "keep\nkeep2\n";
        let hunks = compute_line_diff(old, new);
        assert_eq!(hunks.len(), 1, "contiguous deletes → one hunk");
        let h = &hunks[0];
        assert_eq!(h.removed_count(), 3);
        assert_eq!(h.added_count(), 0);
        assert_eq!(removed_lines(h), vec!["del1", "del2", "del3"]);
    }

    // T5: Full-file replacement — one hunk covering the whole file.
    #[test]
    fn full_file_replacement_produces_one_hunk() {
        let old = "a\nb\nc\n";
        let new = "x\ny\nz\n";
        let hunks = compute_line_diff(old, new);
        assert_eq!(hunks.len(), 1);
        let h = &hunks[0];
        assert_eq!(h.removed_count(), 3);
        assert_eq!(h.added_count(), 3);
    }

    // T6: Changes separated by > 2*CONTEXT lines produce two distinct hunks.
    #[test]
    fn distant_changes_produce_two_hunks() {
        let mut lines_old: Vec<String> = (0..20).map(|i| format!("line-{i}")).collect();
        let mut lines_new = lines_old.clone();
        lines_old[2] = "old-2".to_string();
        lines_new[2] = "new-2".to_string();
        lines_old[17] = "old-17".to_string();
        lines_new[17] = "new-17".to_string();
        let old = lines_old.join("\n");
        let new = lines_new.join("\n");
        let hunks = compute_line_diff(&old, &new);
        assert_eq!(hunks.len(), 2, "well-separated changes → two hunks");
    }

    // T7: to_section_projection produces a well-formed section with one chunk.
    #[test]
    fn to_section_projection_produces_valid_section() {
        let old = "fn main() {}\n";
        let new = "fn main() {\n    println!(\"hello\");\n}\n";
        let hunks = compute_line_diff(old, new);
        assert!(!hunks.is_empty());
        let section = hunks[0].to_section_projection(
            ProposalId(42),
            0,
            None,
            Some(CanonicalPath("src/main.rs".to_string())),
        );
        assert_eq!(section.proposal_id, ProposalId(42));
        assert!(section.section_id.contains("42"));
        assert_eq!(section.title, "main.rs");
        assert!(!section.chunks.is_empty());
        let chunk = &section.chunks[0];
        assert!(chunk.inserted_line_count > 0 || chunk.deleted_line_count > 0);
    }

    // T8: diff_hunks_to_section_projection with multiple hunks.
    #[test]
    fn diff_hunks_to_section_projection_multi_hunk() {
        let mut lines_old: Vec<String> = (0..20).map(|i| format!("line-{i}")).collect();
        let mut lines_new = lines_old.clone();
        lines_old[1] = "old-1".to_string();
        lines_new[1] = "new-1".to_string();
        lines_old[18] = "old-18".to_string();
        lines_new[18] = "new-18".to_string();
        let old = lines_old.join("\n");
        let new = lines_new.join("\n");
        let hunks = compute_line_diff(&old, &new);
        let section = diff_hunks_to_section_projection(
            &hunks,
            ProposalId(1),
            0,
            None,
            None,
            Some("target-A".to_string()),
        );
        assert_eq!(section.chunks.len(), hunks.len());
        for chunk in &section.chunks {
            assert_eq!(chunk.target_id.as_deref(), Some("target-A"));
        }
    }
}
