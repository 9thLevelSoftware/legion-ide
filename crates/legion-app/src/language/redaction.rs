//! LSP stderr redaction (design §8, LANG.11).
//!
//! Summarizes LSP server stderr into metadata-only counts to protect secrets
//! while retaining diagnostic insight.

/// Metadata-only summary of LSP stderr (design §8, LANG.11).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StderrSummary {
    /// Total line count.
    pub line_count: u32,
    /// Lines containing an error marker.
    pub error_lines: u32,
    /// Lines containing a warning marker.
    pub warn_lines: u32,
}

/// Summarizes stderr into counts. Never retains raw line text.
pub fn redact_lsp_stderr(raw: &str) -> StderrSummary {
    let mut line_count = 0;
    let mut error_lines = 0;
    let mut warn_lines = 0;
    for line in raw.lines() {
        line_count += 1;
        let upper = line.to_ascii_uppercase();
        // First-match-wins (intentional): a line matching both ERROR and WARN
        // is counted as an error only.
        if upper.contains("ERROR") {
            error_lines += 1;
        } else if upper.contains("WARN") {
            warn_lines += 1;
        }
    }
    StderrSummary { line_count, error_lines, warn_lines }
}
