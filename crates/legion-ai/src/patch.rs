//! Exact-match patch resolution for model-authored edits.
//!
//! Small models describe edits as *fragments* — "replace this text with that"
//! — while Legion's edit tool takes a file's complete new content. Bridging
//! that naively is destructive: forwarding the new fragment as the whole file
//! deletes everything else. This module resolves a fragment against real file
//! content instead, so an edit either lands exactly where the model meant or
//! is refused with a diagnostic it can act on.
//!
//! Matching is deliberately **exact and unique**:
//!
//! * No match, or more than one, is a refusal — never a guess. Two candidate
//!   sites mean the model was ambiguous, and picking one silently would edit
//!   the wrong line.
//! * Whitespace and line-ending drift do **not** match. A tab-vs-spaces or
//!   CRLF-vs-LF near-miss is reported as a no-match with a diagnostic, because
//!   "close enough" on indentation is how a patch lands in the wrong scope.
//!
//! A refusal carries the nearest candidate line and a similarity score, which
//! is what lets a model re-read and retry rather than escalating to rewriting
//! the file.
//!
//! Behavior and the fixture corpus derive from SmallCode
//! (<https://github.com/Doorman11991/smallcode>, MIT) — see
//! `THIRD_PARTY_NOTICES.md` and `docs/legal/smallcode-attribution.md`.

use serde_json::Value;

/// One model-authored edit: replace `old_str` with `new_str` in `path`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditBlock {
    /// Workspace-relative path the model named.
    pub path: String,
    /// Text to find. Empty means "create this file".
    pub old_str: String,
    /// Replacement text. Empty means "delete the matched text".
    pub new_str: String,
}

/// How an edit was resolved against file content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditResolutionOutcome {
    /// Matched exactly once.
    Exact,
    /// Applied as a whole-file replacement because no fragment was given.
    WholeFileFallback,
}

/// Why an edit could not be resolved, phrased for the model to act on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolutionDiagnostic {
    /// 1-based line of the closest candidate, when one was found.
    pub nearest_line: Option<usize>,
    /// Similarity of that candidate, 0-100.
    pub similarity_percent: u32,
    /// Human- and model-readable explanation.
    pub message: String,
}

/// Result of resolving one edit against file content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchResolution {
    /// The edit resolved; `content` is the file's new full text.
    Applied {
        /// Complete new file content.
        content: String,
        /// How the match was obtained.
        outcome: EditResolutionOutcome,
    },
    /// `old_str` was not present.
    NoMatch(ResolutionDiagnostic),
    /// `old_str` matched more than once, so the target is ambiguous.
    Ambiguous {
        /// Number of occurrences found.
        occurrences: usize,
    },
    /// The edit itself was malformed (missing or non-string fields).
    ValidationError {
        /// What was wrong.
        reason: String,
    },
}

/// Apply one exact-match edit to `file_content`.
pub fn apply_edit(file_content: &str, old_str: &str, new_str: &str) -> PatchResolution {
    if old_str.is_empty() {
        // An empty anchor is only meaningful for a file that does not exist
        // yet. Against existing content it would mean "replace everything" —
        // and a model attempting an insertion by leaving the anchor blank
        // would silently destroy the file. Whole-file rewrites must say so by
        // using `replacement`.
        if !file_content.is_empty() {
            return PatchResolution::ValidationError {
                reason: "`old_str` is empty, which would replace the file's entire contents. \
                         Quote the exact text to replace, or pass `replacement` to rewrite the \
                         whole file deliberately."
                    .to_string(),
            };
        }
        return PatchResolution::Applied {
            content: new_str.to_string(),
            outcome: EditResolutionOutcome::WholeFileFallback,
        };
    }
    match count_overlapping(file_content, old_str) {
        1 => PatchResolution::Applied {
            content: file_content.replacen(old_str, new_str, 1),
            outcome: EditResolutionOutcome::Exact,
        },
        0 => PatchResolution::NoMatch(no_match_diagnostic(file_content, old_str)),
        many => PatchResolution::Ambiguous { occurrences: many },
    }
}

/// Count every position the anchor could start at, including overlapping ones.
///
/// `str::matches` counts non-overlapping occurrences, which under-reports a
/// self-overlapping anchor: `"aa"` in `"aaa"` starts at two positions but
/// counts as one, and the edit would then be applied at the first site under
/// a uniqueness guarantee that does not hold.
fn count_overlapping(haystack: &str, needle: &str) -> usize {
    if needle.is_empty() {
        return 0;
    }
    let mut count = 0usize;
    let mut from = 0usize;
    while let Some(offset) = haystack[from..].find(needle) {
        count += 1;
        let start = from + offset;
        // Advance one character, not one match, so overlaps are seen.
        from = start
            + haystack[start..]
                .chars()
                .next()
                .map(char::len_utf8)
                .unwrap_or(1);
        if from >= haystack.len() {
            break;
        }
    }
    count
}

/// Resolve an edit from a tool call's raw arguments, validating types first.
///
/// Separate from [`apply_edit`] because a model can omit a field or send a
/// number where a string belongs, and that is a different failure from "the
/// text was not found" — the model needs to be told which.
pub fn apply_edit_from_arguments(file_content: &str, arguments: &Value) -> PatchResolution {
    let Value::Object(object) = arguments else {
        return PatchResolution::ValidationError {
            reason: "edit arguments must be an object".to_string(),
        };
    };
    // Accept the spellings models actually use: `old_str`/`new_str`,
    // `old_string`/`new_string`, and Aider-style `search`/`replace`.
    let old_str = match object
        .get("old_str")
        .or_else(|| object.get("old_string"))
        .or_else(|| object.get("search"))
    {
        Some(Value::String(text)) => text.clone(),
        Some(_) => {
            return PatchResolution::ValidationError {
                reason: "`old_str` must be a string".to_string(),
            };
        }
        None => {
            return PatchResolution::ValidationError {
                reason: "`old_str` is required".to_string(),
            };
        }
    };
    let new_str = match object
        .get("new_str")
        .or_else(|| object.get("new_string"))
        .or_else(|| object.get("replace"))
    {
        Some(Value::String(text)) => text.clone(),
        Some(_) => {
            return PatchResolution::ValidationError {
                reason: "`new_str` must be a string".to_string(),
            };
        }
        None => {
            return PatchResolution::ValidationError {
                reason: "`new_str` is required".to_string(),
            };
        }
    };
    apply_edit(file_content, &old_str, &new_str)
}

/// Build the refusal diagnostic for a fragment that was not found.
///
/// Names the closest line so the model can re-read that region instead of
/// guessing, and calls out whitespace or line-ending drift explicitly, since
/// those are the near-misses that look identical in a chat transcript.
fn no_match_diagnostic(file_content: &str, old_str: &str) -> ResolutionDiagnostic {
    let needle_first = old_str.lines().next().unwrap_or(old_str).trim();
    let (best_line, best_score) = nearest_candidate_line(file_content, needle_first);

    let mut hints = Vec::new();
    if normalize_whitespace(file_content).contains(&normalize_whitespace(old_str)) {
        // Deliberately says "whitespace" rather than "indentation": the same
        // check fires when the model collapsed a line break or changed inner
        // spacing, and naming only indentation sends it looking in the wrong
        // place.
        hints.push("the text is present but its whitespace differs (indentation, line breaks, or spacing between tokens)");
    }
    if file_content.contains("\r\n") && !old_str.contains("\r\n") {
        hints.push("the file uses CRLF line endings and the search text uses LF");
    }

    let mut message = String::from("`old_str` was not found in the file exactly as written");
    if let Some(line) = best_line {
        message.push_str(&format!("; closest line is {line} ({best_score}% similar)"));
    }
    for hint in hints {
        message.push_str("; ");
        message.push_str(hint);
    }
    message.push_str(". Re-read the file and quote the text exactly, including indentation.");

    ResolutionDiagnostic {
        nearest_line: best_line,
        similarity_percent: best_score,
        message,
    }
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Borrow at most `max_chars` characters, cutting on a character boundary.
fn char_prefix(text: &str, max_chars: usize) -> &str {
    match text.char_indices().nth(max_chars) {
        Some((offset, _)) => &text[..offset],
        None => text,
    }
}

/// Find the line a model should re-read, with a confidence score.
///
/// Ordered by how strong the signal is: a line containing the needle outright,
/// then the best shared prefix. Prefix overlap is used rather than character
/// counting because position matters — `abc` and `cab` share every character
/// but are not the same code, and pointing at the wrong line is worse than
/// admitting low confidence. Single pass over the file.
fn nearest_candidate_line(file_content: &str, needle_first: &str) -> (Option<usize>, u32) {
    if needle_first.is_empty() {
        return (None, 0);
    }
    // Diagnostics are advisory, and this runs on the failure path where the
    // file may be minified — a single megabyte-long line against a long anchor
    // would otherwise cost billions of comparisons just to build retry
    // feedback. Comparing bounded samples points at the same line.
    const SAMPLE_CHARS: usize = 256;
    let needle_sample = char_prefix(needle_first, SAMPLE_CHARS);

    let mut best_line = None;
    let mut best_score = 0u32;
    for (index, line) in file_content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let trimmed = char_prefix(trimmed, SAMPLE_CHARS);
        let needle_first = needle_sample;
        // Containment either way is the strongest available signal: the model
        // quoted a slice of this line, or padded around it.
        if trimmed.contains(needle_first) || needle_first.contains(trimmed) {
            return (Some(index + 1), 100);
        }
        let shared = trimmed
            .chars()
            .zip(needle_first.chars())
            .take_while(|(a, b)| a == b)
            .count();
        if shared == 0 {
            continue;
        }
        let longest = trimmed.chars().count().max(needle_first.chars().count());
        let score = ((shared * 100) / longest.max(1)) as u32;
        if score > best_score {
            best_score = score;
            best_line = Some(index + 1);
        }
    }
    (best_line, best_score)
}

// ---------------------------------------------------------------------------
// Block parsing
// ---------------------------------------------------------------------------

const SEARCH_MARKER: &str = "<<<<<<< SEARCH";
const DIVIDER_MARKER: &str = "=======";
const REPLACE_MARKER: &str = ">>>>>>> REPLACE";

/// Parse edit blocks out of model prose.
///
/// Understands the two formats models reach for: conflict-style
/// `<<<<<<< SEARCH` / `=======` / `>>>>>>> REPLACE` blocks preceded by a path
/// line, and unified-diff hunks inside a ```diff fence. A block missing its
/// divider or terminator yields nothing — a half-written edit is not a
/// partially-valid one.
pub fn parse_edit_blocks(text: &str) -> Vec<EditBlock> {
    let mut blocks = parse_search_replace_blocks(text);
    blocks.extend(parse_diff_fences(text));
    blocks
}

fn parse_search_replace_blocks(text: &str) -> Vec<EditBlock> {
    let lines: Vec<&str> = text.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        if lines[index].trim() != SEARCH_MARKER {
            index += 1;
            continue;
        }
        // The path is the nearest non-empty line above the marker.
        let path = lines[..index]
            .iter()
            .rev()
            .find(|line| !line.trim().is_empty())
            .map(|line| line.trim().to_string())
            .unwrap_or_default();

        let mut old_lines = Vec::new();
        let mut cursor = index + 1;
        let mut saw_divider = false;
        while cursor < lines.len() {
            if lines[cursor].trim() == DIVIDER_MARKER {
                saw_divider = true;
                break;
            }
            if lines[cursor].trim() == REPLACE_MARKER {
                break;
            }
            old_lines.push(lines[cursor]);
            cursor += 1;
        }
        if !saw_divider {
            // No divider: the search and replace halves cannot be told apart.
            index = cursor + 1;
            continue;
        }

        let mut new_lines = Vec::new();
        cursor += 1;
        let mut saw_terminator = false;
        while cursor < lines.len() {
            if lines[cursor].trim() == REPLACE_MARKER {
                saw_terminator = true;
                break;
            }
            new_lines.push(lines[cursor]);
            cursor += 1;
        }
        if !saw_terminator {
            // Truncated mid-block: the replacement text may be incomplete, and
            // applying a partial replacement would silently truncate the file.
            index = cursor + 1;
            continue;
        }

        if !path.is_empty() {
            blocks.push(EditBlock {
                path,
                old_str: old_lines.join("\n"),
                new_str: new_lines.join("\n"),
            });
        }
        index = cursor + 1;
    }
    blocks
}

fn parse_diff_fences(text: &str) -> Vec<EditBlock> {
    let mut blocks = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find("```diff") {
        let body_start = match rest[open..].find('\n') {
            Some(offset) => open + offset + 1,
            None => break,
        };
        // A hunk without its closing fence may be cut mid-line; reconstructing
        // from it would drop whatever the model had not finished writing.
        let Some(close_offset) = rest[body_start..].find("```") else {
            break;
        };
        let body = &rest[body_start..body_start + close_offset];
        if let Some(block) = parse_unified_hunk(body) {
            blocks.push(block);
        }
        rest = &rest[body_start + close_offset + 3..];
    }
    blocks
}

fn parse_unified_hunk(body: &str) -> Option<EditBlock> {
    let mut path = None::<String>;
    let mut old_lines = Vec::new();
    let mut new_lines = Vec::new();
    let mut in_hunk = false;

    for line in body.lines() {
        if let Some(target) = line.strip_prefix("+++ ") {
            let target = target.trim();
            let target = target.strip_prefix("b/").unwrap_or(target);
            if target != "/dev/null" {
                path = Some(target.to_string());
            }
            continue;
        }
        if line.starts_with("--- ") {
            continue;
        }
        if line.starts_with("@@") {
            in_hunk = true;
            continue;
        }
        if !in_hunk {
            continue;
        }
        match line.as_bytes().first() {
            Some(b'-') => old_lines.push(line[1..].to_string()),
            Some(b'+') => new_lines.push(line[1..].to_string()),
            Some(b' ') => {
                old_lines.push(line[1..].to_string());
                new_lines.push(line[1..].to_string());
            }
            // A bare empty line inside a hunk is context.
            None => {
                old_lines.push(String::new());
                new_lines.push(String::new());
            }
            _ => continue,
        }
    }

    let path = path?;
    if old_lines.is_empty() && new_lines.is_empty() {
        return None;
    }
    Some(EditBlock {
        path,
        old_str: old_lines.join("\n"),
        new_str: new_lines.join("\n"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unique_match_applies_in_place() {
        let out = apply_edit(
            "fn main() {\n    println!(\"a\");\n}\n",
            "println!(\"a\");",
            "println!(\"b\");",
        );
        assert_eq!(
            out,
            PatchResolution::Applied {
                content: "fn main() {\n    println!(\"b\");\n}\n".to_string(),
                outcome: EditResolutionOutcome::Exact,
            }
        );
    }

    #[test]
    fn ambiguity_is_refused_rather_than_guessed() {
        // Two candidate sites: picking one silently would edit the wrong line.
        let out = apply_edit("x = 1\nx = 1\n", "x = 1", "x = 2");
        assert_eq!(out, PatchResolution::Ambiguous { occurrences: 2 });
    }

    #[test]
    fn indentation_drift_does_not_match_but_is_explained() {
        let out = apply_edit(
            "def f():\n    return 1\n",
            "def f():\n\treturn 1",
            "def f():\n\treturn 2",
        );
        match out {
            PatchResolution::NoMatch(diagnostic) => {
                assert!(
                    diagnostic.message.contains("whitespace differs"),
                    "diagnostic should name the drift: {}",
                    diagnostic.message
                );
                assert!(diagnostic.nearest_line.is_some());
            }
            other => panic!("expected NoMatch, got {other:?}"),
        }
    }

    #[test]
    fn line_ending_drift_does_not_match_but_is_explained() {
        let out = apply_edit(
            "line one\r\nline two\r\n",
            "line one\nline two",
            "line 1\nline 2",
        );
        match out {
            PatchResolution::NoMatch(diagnostic) => assert!(
                diagnostic.message.contains("CRLF"),
                "diagnostic should name the line-ending mismatch: {}",
                diagnostic.message
            ),
            other => panic!("expected NoMatch, got {other:?}"),
        }
    }

    #[test]
    fn missing_or_mistyped_fields_are_validation_errors() {
        let missing = apply_edit_from_arguments("abc\n", &serde_json::json!({"old_str": "abc"}));
        assert!(matches!(missing, PatchResolution::ValidationError { .. }));

        let mistyped = apply_edit_from_arguments(
            "count = 41\n",
            &serde_json::json!({"old_str": "41", "new_str": 42}),
        );
        assert!(matches!(mistyped, PatchResolution::ValidationError { .. }));
    }

    #[test]
    fn empty_search_creates_a_file_but_never_overwrites_one() {
        // Creating a file: nothing to anchor to, so the content stands alone.
        assert_eq!(
            apply_edit("", "", "pub fn hello() {}"),
            PatchResolution::Applied {
                content: "pub fn hello() {}".to_string(),
                outcome: EditResolutionOutcome::WholeFileFallback,
            }
        );

        // Against an existing file, an empty anchor would mean "replace
        // everything" — a model attempting an insertion this way would destroy
        // the file, so it is refused and told which field to use instead.
        match apply_edit("existing content\n", "", "inserted") {
            PatchResolution::ValidationError { reason } => assert!(
                reason.contains("replacement"),
                "should point at the deliberate whole-file field: {reason}"
            ),
            other => panic!("expected ValidationError, got {other:?}"),
        }
    }

    #[test]
    fn self_overlapping_anchors_count_as_ambiguous() {
        // "aa" starts at offsets 0 and 1 in "aaa". `str::matches` reports one
        // occurrence, which would apply the edit under a uniqueness guarantee
        // that does not actually hold.
        assert_eq!(
            apply_edit("aaa", "aa", "bb"),
            PatchResolution::Ambiguous { occurrences: 2 }
        );
        // A genuinely unique anchor still applies.
        assert!(matches!(
            apply_edit("abab\n", "abab", "cd"),
            PatchResolution::Applied { .. }
        ));
    }

    #[test]
    fn diagnostics_stay_bounded_on_minified_input() {
        // One very long line plus a long anchor: the failure path must not
        // turn retry feedback into a stall.
        let content = format!("{}\n", "x".repeat(200_000));
        let anchor = "y".repeat(50_000);
        let started = std::time::Instant::now();
        let out = apply_edit(&content, &anchor, "z");
        assert!(matches!(out, PatchResolution::NoMatch(_)));
        assert!(
            started.elapsed() < std::time::Duration::from_secs(2),
            "diagnostic construction took {:?}",
            started.elapsed()
        );
    }

    #[test]
    fn half_written_blocks_yield_nothing() {
        // Missing divider: the two halves cannot be told apart.
        assert!(
            parse_edit_blocks(
                "src/lib.rs\n<<<<<<< SEARCH\nfn a() {}\nfn b() {}\n>>>>>>> REPLACE\n"
            )
            .is_empty()
        );
        // Truncated: the replacement text may be incomplete.
        assert!(
            parse_edit_blocks("src/lib.rs\n<<<<<<< SEARCH\nfn a() {}\n=======\nfn b() {}\n")
                .is_empty()
        );
        // Truncated diff fence.
        assert!(
            parse_edit_blocks("```diff\n--- a/x.rs\n+++ b/x.rs\n@@ -1,2 +1,2 @@\n line\n-old li")
                .is_empty()
        );
    }
}
