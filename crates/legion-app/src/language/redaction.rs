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

/// Scan `input` token by token and replace any token that looks like an
/// absolute file-system path with `[REDACTED]`.
///
/// Path heuristics:
/// - Windows: a single ASCII alpha char immediately followed by `:` then
///   `\` or `/`, at a word boundary (preceded by a non-alphanumeric char or
///   start of string).  Using `!is_alphanumeric` instead of `is_whitespace`
///   catches paths adjacent to punctuation such as backticks, parentheses,
///   and colons (e.g. `` `C:\path` `` in rustc error messages) while still
///   avoiding false-positive matches inside identifiers or URLs like
///   `https://` (the `chars[i+1] == ':'` + `chars[i+2] == '\\' or '/'`
///   guard already excludes `http://` since `p` in `https` is alphabetic
///   but the following character is `t`, not `:`).
/// - UNC paths: starts with `\\` — consumed in full as an absolute path.
/// - Unix absolute path: `/` not followed by another `/` (avoids matching
///   `//` double-slash sequences in non-path contexts).
/// - Home-relative: `~/` — a common Unix shorthand for `$HOME`.
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

        // Determine whether this is a word boundary: any position where the
        // preceding character is NOT alphanumeric (covers whitespace, backtick,
        // parenthesis, colon, etc.).  This catches paths adjacent to
        // punctuation in rust-analyzer error output, e.g.
        //   `error in \`C:\Users\ws\src\main.rs\``
        //   `error (C:\Users\ws\main.rs) ...`
        let at_word_start = i == 0 || !chars[i - 1].is_alphanumeric();

        // UNC path: starts with `\\` (two backslashes).
        let is_unc_path = i + 1 < chars.len() && chars[i] == '\\' && chars[i + 1] == '\\';

        // Windows drive-letter path: alpha + colon + (back)slash at a word
        // boundary.  The `at_word_start` guard prevents matching inside
        // identifiers; `chars[i+2] == '\\' or '/'` prevents matching labels
        // such as `option: value`.
        let is_windows_path = at_word_start
            && i + 2 < chars.len()
            && chars[i].is_ascii_alphabetic()
            && chars[i + 1] == ':'
            && (chars[i + 2] == '\\' || chars[i + 2] == '/');

        // Unix absolute path: `/` not followed by another `/`.
        let is_unix_path = chars[i] == '/' && i + 1 < chars.len() && chars[i + 1] != '/';

        // Home-relative path: `~/`.
        let is_home_path = chars[i] == '~' && i + 1 < chars.len() && chars[i + 1] == '/';

        if is_unc_path || is_windows_path || is_unix_path || is_home_path {
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
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
        // Non-path text must be preserved.
        assert!(
            out.contains("error in"),
            "non-path prefix must be preserved: {out}"
        );
        assert!(
            out.contains("syntax error"),
            "non-path suffix must be preserved: {out}"
        );
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
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
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
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
    }

    // ── T4-R5: Backtick-adjacent Windows path (I-4 fix) ──────────────────────
    //
    // rust-analyzer emits errors such as:
    //   error[E0583]: file `C:\Users\corp\workspace\main.rs` not found
    // The `C` is preceded by a backtick, which is not whitespace.
    // Before the I-4 fix, `at_word_start` was false here and the path leaked.

    #[test]
    fn t4_backtick_adjacent_windows_path_is_redacted() {
        let line = r"error[E0583]: file `C:\Users\corp\ws\main.rs` not found";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("Users"),
            "raw path segment must not appear: {out}"
        );
        assert!(
            !out.contains("main.rs"),
            "raw filename must not appear: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
        // Non-path text must be preserved.
        assert!(
            out.contains("error[E0583]"),
            "error code must be preserved: {out}"
        );
        assert!(
            out.contains("not found"),
            "suffix text must be preserved: {out}"
        );
    }

    // ── T4-R6: Paren-adjacent Windows path (I-4 fix) ─────────────────────────

    #[test]
    fn t4_paren_adjacent_windows_path_is_redacted() {
        let line = r"error at (C:\Users\corp\ws\src\lib.rs:42)";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("Users"),
            "raw path segment must not appear: {out}"
        );
        assert!(
            !out.contains("lib.rs"),
            "raw filename must not appear: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
    }

    // ── T4-R7: Colon-adjacent Windows path (I-4 fix) ─────────────────────────

    #[test]
    fn t4_colon_adjacent_windows_path_is_redacted() {
        let line = r"error in:C:\Users\corp\ws\src\main.rs";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("Users"),
            "raw path segment must not appear: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
    }

    // ── T4-R8: UNC path (I-3 fix) ────────────────────────────────────────────
    //
    // A workspace on a network share emits lines such as:
    //   error reading \\corp\projects\ws\src\lib.rs

    #[test]
    fn t4_unc_path_is_redacted() {
        let line = r"error reading \\corp\projects\ws\src\lib.rs: file not found";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("corp"),
            "UNC server name must not appear: {out}"
        );
        assert!(
            !out.contains("projects"),
            "UNC share name must not appear: {out}"
        );
        assert!(
            !out.contains("lib.rs"),
            "raw filename must not appear: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
        assert!(
            out.contains("error reading"),
            "non-path prefix must be preserved: {out}"
        );
        assert!(
            out.contains("file not found"),
            "non-path suffix must be preserved: {out}"
        );
    }

    // ── T4-R9: Home-relative path (m-2 fix) ──────────────────────────────────

    #[test]
    fn t4_home_relative_path_is_redacted() {
        let line = "analyzing ~/projects/ws/src/main.rs";
        let out = redact_lsp_stderr_line(line);
        assert!(
            !out.contains("projects"),
            "home-relative path segment must not appear: {out}"
        );
        assert!(
            !out.contains("main.rs"),
            "raw filename must not appear: {out}"
        );
        assert!(
            out.contains("[REDACTED]"),
            "redaction marker must appear: {out}"
        );
        assert!(
            out.contains("analyzing"),
            "non-path prefix must be preserved: {out}"
        );
    }
}
