//! LSP stderr redaction (design §8, LANG.11).
//!
//! Summarizes LSP server stderr into metadata-only counts to protect secrets
//! while retaining diagnostic insight.
//!
//! `redact_lsp_stderr_line` further provides per-line path redaction for the
//! ring-buffer projection (PKT-LSP-C T4): absolute file paths (Windows and
//! Unix) are replaced with `[REDACTED]` so diagnostic text can be projected
//! to the UI without exposing workspace paths or file content.

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
    StderrSummary {
        line_count,
        error_lines,
        warn_lines,
    }
}

/// Redacts a single LSP stderr line for safe projection (PKT-LSP-C T4).
///
/// Replaces absolute file system paths (Windows: `C:\…` / `C:/…`, Unix:
/// `/foo/…`) with `[REDACTED]`.  Non-path diagnostic text (log levels,
/// error messages, module names) is preserved.  Metadata-only: no absolute
/// path escapes the redaction gate.
pub fn redact_lsp_stderr_line(line: &str) -> String {
    redact_paths(line)
}

/// Scan `input` token by token (splitting on whitespace boundaries) and
/// replace any token that looks like an absolute file-system path with
/// `[REDACTED]`.
///
/// Path heuristics:
/// - Windows: a single ASCII alpha char immediately followed by `:` then
///   `\` or `/`, at a word boundary (preceded by whitespace or start of
///   string).  This avoids false-positive matches on identifiers whose
///   second char is `:`.
/// - Unix: a `/` that is NOT followed by another `/` (to avoid matching
///   double-slash sequences in non-path contexts).
fn redact_paths(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Preserve whitespace exactly.
        if chars[i].is_whitespace() {
            result.push(chars[i]);
            i += 1;
            continue;
        }

        // Determine whether this is a word boundary (previous char was
        // whitespace or we are at the start of the string).
        let at_word_start = i == 0 || chars[i - 1].is_whitespace();

        // Windows path: single alpha + colon + (back)slash at a word boundary.
        let is_windows_path = at_word_start
            && i + 2 < chars.len()
            && chars[i].is_ascii_alphabetic()
            && chars[i + 1] == ':'
            && (chars[i + 2] == '\\' || chars[i + 2] == '/');

        // Unix absolute path: slash not followed by another slash.
        let is_unix_path = chars[i] == '/'
            && i + 1 < chars.len()
            && chars[i + 1] != '/';

        if is_windows_path || is_unix_path {
            // Consume the entire non-whitespace token (the path).
            while i < chars.len() && !chars[i].is_whitespace() {
                i += 1;
            }
            result.push_str("[REDACTED]");
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

// ─────────────────────────────────────────────────────────────────────────────
// PKT-LSP-C T4: redact_lsp_stderr_line — TDD tests
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod t4_redaction_tests {
    use super::*;

    // ── T4-R1: Windows path stripped ─────────────────────────────────────────

    #[test]
    fn t4_redact_line_strips_windows_path() {
        let line = r"error in C:\Users\foo\project\src\main.rs: syntax error";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("Users"),
            "raw Windows path segment must not appear: {out}"
        );
        assert!(
            !out.contains("main.rs"),
            "raw filename must not appear: {out}"
        );
        assert!(out.contains("[REDACTED]"), "redaction marker must appear: {out}");
        // Non-path text must be preserved.
        assert!(out.contains("error in"), "non-path prefix must be preserved: {out}");
        assert!(out.contains("syntax error"), "non-path suffix must be preserved: {out}");
    }

    // ── T4-R2: Unix path stripped ─────────────────────────────────────────────

    #[test]
    fn t4_redact_line_strips_unix_path() {
        let line = "thread panicked at /home/user/project/src/main.rs:42:5";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("home"),
            "raw Unix path segment must not appear: {out}"
        );
        assert!(
            !out.contains("main.rs"),
            "raw filename must not appear: {out}"
        );
        assert!(out.contains("[REDACTED]"), "redaction marker must appear: {out}");
        assert!(
            out.contains("thread panicked at"),
            "non-path prefix must be preserved: {out}"
        );
    }

    // ── T4-R3: Non-path text left intact ─────────────────────────────────────

    #[test]
    fn t4_redact_line_leaves_non_path_text() {
        let line = "ERROR: connection refused (rust-analyzer)";
        let out = redact_lsp_stderr_line(line);
        assert_eq!(out, line, "non-path line must be returned unchanged");
    }

    // ── T4-R4: Sentinel secret never escapes ─────────────────────────────────

    #[test]
    fn t4_sentinel_not_in_redacted_output() {
        let sentinel = "/secret/path/to/workspace/src/lib.rs";
        let line = format!("rust-analyzer loaded {sentinel} successfully");
        let out = redact_lsp_stderr_line(&line);
        assert!(
            !out.contains(sentinel),
            "sentinel secret must not appear in redacted output: {out}"
        );
        assert!(
            !out.contains("secret"),
            "any fragment of the sentinel must not appear: {out}"
        );
        assert!(out.contains("[REDACTED]"), "redaction marker must appear: {out}");
    }
}
