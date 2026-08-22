//! Exact-match patch resolution for model-authored edits.
//!
//! Small models describe edits as *fragments* — "replace this text with that"
//! — while Legion's edit tool takes a file's complete new content. Bridging
//! that naively is destructive: forwarding the new fragment as the whole file
//! deletes everything else. This module resolves a fragment against real file
//! content instead, so an edit either lands exactly where the model meant or
//! is refused with a diagnostic it can act on.
//!
//! Matching is **unique or nothing**, in three stages, each tried only after
//! the one before it finds nothing:
//!
//! 1. Exact.
//! 2. Whitespace-tolerant: indentation and inter-token spacing ignored.
//! 3. Block-anchored, for replacement text supplied with no anchor at all.
//!
//! At every stage, more than one candidate is a refusal — never a guess. Two
//! sites mean the model was ambiguous, and picking one silently would edit the
//! wrong line. And the tolerance is only in the *search*: the bytes replaced
//! are always the file's own, so an applied edit is exact regardless of how it
//! was located.
//!
//! Stages 2 and 3 are a deliberate relaxation of an earlier exact-only policy,
//! made after measuring what a small model actually gets wrong. Refusing a
//! whitespace near-miss was correct about the anchor and useless to the model,
//! which rewrote the same anchor and failed again; on qwen2.5-coder:7b those
//! two failures accounted for most rejected edits
//! (`plans/evidence/production/BENCH/baseline-raw-v1.md`).
//!
//! A refusal still carries the nearest candidate line and a similarity score,
//! which is what lets a model re-read and retry rather than escalating to
//! rewriting the file.
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
    /// Located by ignoring indentation and inter-token spacing, then applied
    /// to the file's own bytes.
    ///
    /// Distinct from [`Exact`](Self::Exact) so review can see that the anchor
    /// the model wrote did not literally appear in the file — the edit is
    /// still byte-exact, but the model was working from an approximation.
    Fuzzy,
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
    match resolve_edit_span(file_content, old_str, new_str) {
        PatchSpan::Resolved {
            start,
            end,
            replacement,
            outcome,
        } => PatchResolution::Applied {
            content: {
                let mut content = String::with_capacity(file_content.len() + replacement.len());
                content.push_str(&file_content[..start]);
                content.push_str(&replacement);
                content.push_str(&file_content[end..]);
                content
            },
            outcome,
        },
        PatchSpan::NoMatch(diagnostic) => PatchResolution::NoMatch(diagnostic),
        PatchSpan::Ambiguous { occurrences } => PatchResolution::Ambiguous { occurrences },
        PatchSpan::ValidationError { reason } => PatchResolution::ValidationError { reason },
    }
}

/// Where an edit lands, as a byte span in the file it was resolved against.
///
/// The same decision as [`PatchResolution`], reported as a span instead of a
/// rewritten file. A caller that has to produce a reviewable edit needs the
/// span: a `TextEdit` covering the whole file says "everything changed", which
/// is both false and unreadable in a diff, and it makes a one-line change
/// impossible to approve on sight.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchSpan {
    /// The edit resolved to `file_content[start..end]`.
    Resolved {
        /// Byte offset the replacement starts at.
        start: usize,
        /// Byte offset the replacement ends at.
        end: usize,
        /// Text to put in that span, with the trailing-newline rule applied.
        replacement: String,
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
    /// The edit itself was malformed.
    ValidationError {
        /// What was wrong.
        reason: String,
    },
}

/// Resolve one edit to the span it occupies in `file_content`.
///
/// This is the search; [`apply_edit`] is this plus a splice. Written in that
/// order rather than the reverse because two copies of "exact, then
/// whitespace-tolerant, then refuse" is one copy that eventually disagrees
/// with the other about what counts as a match.
pub fn resolve_edit_span(file_content: &str, old_str: &str, new_str: &str) -> PatchSpan {
    if old_str.is_empty() {
        // An empty anchor is only meaningful for a file that does not exist
        // yet. Against existing content it would mean "replace everything" —
        // and a model attempting an insertion by leaving the anchor blank
        // would silently destroy the file. Whole-file rewrites must say so by
        // using `replacement`.
        if !file_content.is_empty() {
            return PatchSpan::ValidationError {
                reason: "`old_str` is empty, which would replace the file's entire contents. \
                         Quote the exact text to replace, or pass `replacement` to rewrite the \
                         whole file deliberately."
                    .to_string(),
            };
        }
        return PatchSpan::Resolved {
            start: 0,
            end: 0,
            replacement: new_str.to_string(),
            outcome: EditResolutionOutcome::WholeFileFallback,
        };
    }
    match count_overlapping(file_content, old_str) {
        1 => {
            let start = file_content
                .find(old_str)
                .expect("a single counted occurrence must be findable");
            let end = start + old_str.len();
            PatchSpan::Resolved {
                start,
                end,
                replacement: newline_adjusted(file_content, start, end, new_str),
                outcome: EditResolutionOutcome::Exact,
            }
        }
        0 => {
            // Exact matching failed. Before refusing, try again ignoring
            // indentation and inter-token spacing: the anchor is written from
            // memory and the spacing is what a small model gets wrong. The
            // span must still be unique, and the bytes replaced are the
            // file's, so the edit remains exact — only the search was
            // tolerant.
            match find_whitespace_insensitive(file_content, old_str) {
                Some((start, end)) => PatchSpan::Resolved {
                    start,
                    end,
                    replacement: newline_adjusted(file_content, start, end, new_str),
                    outcome: EditResolutionOutcome::Fuzzy,
                },
                None => PatchSpan::NoMatch(no_match_diagnostic(file_content, old_str)),
            }
        }
        many => PatchSpan::Ambiguous { occurrences: many },
    }
}

/// Resolve an edit to a span from a tool call's raw arguments.
///
/// The span-returning twin of [`apply_edit_from_arguments`], sharing its
/// argument-spelling tolerance.
pub fn resolve_edit_span_from_arguments(file_content: &str, arguments: &Value) -> PatchSpan {
    match edit_arguments(arguments) {
        Ok((old_str, new_str)) => resolve_edit_span(file_content, &old_str, &new_str),
        Err(reason) => PatchSpan::ValidationError { reason },
    }
}

/// The trailing-newline rule, applied to the replacement rather than at splice
/// time so a caller holding only the span still gets faithful text.
///
/// Replacing a span that ended a line with text that does not would silently
/// join it to the following line.
///
/// An empty replacement is exempt, and that exemption is the whole subtlety.
/// Empty means "delete this span", and a deletion that ended a line takes the
/// line ending with it -- re-adding one leaves a blank line where the text was.
/// The rule used to live only in `splice_replacement`, which the exact-match
/// path never called, so exact deletions came out clean and whitespace-tolerant
/// ones silently left the blank line behind. One rule now, and it knows the
/// difference between replacing and deleting.
fn newline_adjusted(file_content: &str, start: usize, end: usize, new_str: &str) -> String {
    let mut replacement = new_str.to_string();
    if !new_str.is_empty()
        && !new_str.ends_with('\n')
        && file_content[start..end].ends_with('\n')
    {
        replacement.push('\n');
    }
    replacement
}

/// Replace `file_content[start..end]` with `new_str`.
///
/// Shared so the trailing-newline rule lives in one place: replacing a span
/// that ended a line with text that does not would silently join it to the
/// following line. Two copies of that rule is one copy that eventually
/// forgets it.
pub fn splice_replacement(file_content: &str, start: usize, end: usize, new_str: &str) -> String {
    let replacement = newline_adjusted(file_content, start, end, new_str);
    let mut content = String::with_capacity(file_content.len() + replacement.len());
    content.push_str(&file_content[..start]);
    content.push_str(&replacement);
    content.push_str(&file_content[end..]);
    content
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
    match edit_arguments(arguments) {
        Ok((old_str, new_str)) => apply_edit(file_content, &old_str, &new_str),
        Err(reason) => PatchResolution::ValidationError { reason },
    }
}

/// Read `old_str`/`new_str` out of a tool call's arguments.
fn edit_arguments(arguments: &Value) -> Result<(String, String), String> {
    let Value::Object(object) = arguments else {
        return Err("edit arguments must be an object".to_string());
    };
    // Accept the spellings models actually use: `old_str`/`new_str`,
    // `old_string`/`new_string`, and Aider-style `search`/`replace`.
    let old_str = match object
        .get("old_str")
        .or_else(|| object.get("old_string"))
        .or_else(|| object.get("search"))
    {
        Some(Value::String(text)) => text.clone(),
        Some(_) => return Err("`old_str` must be a string".to_string()),
        None => return Err("`old_str` is required".to_string()),
    };
    let new_str = match object
        .get("new_str")
        .or_else(|| object.get("new_string"))
        .or_else(|| object.get("replace"))
    {
        Some(Value::String(text)) => text.clone(),
        Some(_) => return Err("`new_str` must be a string".to_string()),
        None => return Err("`new_str` is required".to_string()),
    };
    Ok((old_str, new_str))
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
        // Policy change, deliberate: this case used to be refused with a
        // diagnostic naming the drift. Refusing was correct about the anchor
        // and useless to the model, which rewrote the same anchor and failed
        // again. Indentation is now tolerated when it resolves to one span.
        match out {
            PatchResolution::Applied { content, outcome } => {
                assert_eq!(outcome, EditResolutionOutcome::Fuzzy);
                assert_eq!(
                    content,
                    "def f():
	return 2
",
                    "the file's own indentation is what gets replaced"
                );
            }
            other => panic!("expected a fuzzy match, got {other:?}"),
        }
    }

    /// The diagnostic that used to fire above still has to work, because an
    /// anchor whose whitespace differs *and* which appears twice is still
    /// refused, and that is when the model most needs to be told why.
    #[test]
    fn drift_that_resolves_ambiguously_is_still_explained() {
        let out = apply_edit(
            "def f():
    return 1

def g():
    return 1
",
            "	return 1",
            "	return 2",
        );
        match out {
            PatchResolution::NoMatch(diagnostic) => {
                assert!(
                    diagnostic.message.contains("whitespace differs"),
                    "diagnostic should still name the drift: {}",
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
        // Also a deliberate policy change. A model reading a CRLF file over a
        // tool boundary that normalizes line endings cannot write a CRLF
        // anchor, so refusing it punished the model for something it could
        // not see.
        match out {
            PatchResolution::Applied { content, outcome } => {
                assert_eq!(outcome, EditResolutionOutcome::Fuzzy);
                assert_eq!(
                    content,
                    "line 1
line 2
"
                );
            }
            other => panic!("expected a fuzzy match, got {other:?}"),
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

// ─── Block anchoring for a replacement given without an anchor ───────────────

/// Infer the region a lone `new_str` was meant to replace.
///
/// Small models frequently answer an edit request with only the *new* text —
/// "here is what the function should be" — and no `old_str`. Observed on
/// qwen2.5-coder:7b in roughly half of its `edit-as-proposal` calls
/// (`plans/evidence/production/BENCH/baseline-raw-v1.md`). The two obvious
/// readings are both wrong: rejecting it wastes a turn the model usually does
/// not recover from, and treating it as the file's complete content silently
/// deletes everything the model did not retype — the whole-file clobber that
/// review caught once already.
///
/// The safe reading is narrower. When `new_str` is a brace-balanced block, its
/// first line names the item it defines; if the file contains that line
/// exactly once, the region to replace is that line through the end of its
/// block. Nothing outside that block is touched.
///
/// Returns the byte range in `file_content` to replace, or `None` when any
/// condition fails — in which case the caller must refuse rather than guess.
pub fn anchor_replacement_block(file_content: &str, new_str: &str) -> Option<(usize, usize)> {
    let first_line = new_str.lines().find(|line| !line.trim().is_empty())?;
    let anchor = first_line.trim();
    // An anchor must be substantial enough to be a definition, not `}` or `{`.
    if anchor.len() < 4 {
        return None;
    }
    // The replacement must be a complete block; an unbalanced one would leave
    // the file unparseable no matter where it landed.
    if brace_balance(new_str) != 0 {
        return None;
    }
    // The anchor line must *syntactically* open a block. Testing for the
    // character instead accepts `const marker = "{";`, whose brace the scanner
    // correctly ignores — the forward scan then sees depth zero on the first
    // line, ends the range there, and the splice keeps whatever followed.
    // The guard has to ask the same question the scan answers.
    if BalanceScan::default().consume(first_line) <= 0 {
        return None;
    }

    // Exactly one line may match, or the target is a guess.
    let mut match_offset = None;
    let mut offset = 0_usize;
    for line in file_content.split_inclusive('\n') {
        if line.trim() == anchor {
            if match_offset.is_some() {
                return None;
            }
            match_offset = Some(offset);
        }
        offset += line.len();
    }
    let start = match_offset?;

    // Walk forward to the end of the block the anchor opens.
    let mut depth = 0_i32;
    let mut end = None;
    let mut cursor = start;
    // The scanner carries state across lines. Balancing each line
    // independently forgets an open `/*`, so a `}` on a later line of a block
    // comment reads as syntax and closes the block early — the scan returns a
    // truncated range and the splice leaves the tail of the old block behind.
    let mut scan = BalanceScan::default();
    for line in file_content[start..].split_inclusive('\n') {
        depth += scan.consume(line);
        cursor += line.len();
        if depth <= 0 {
            end = Some(cursor);
            break;
        }
    }
    // A block that never closes means the anchor sits inside a construct this
    // scan does not understand.
    Some((start, end?))
}

fn brace_balance(text: &str) -> i32 {
    BalanceScan::default().consume(text)
}

/// Brace-counting state that survives between chunks.
///
/// Exists because the block scan feeds one line at a time and a block comment
/// spans lines: restarting the state per line forgets an open `/*`, and a `}`
/// on its second line then reads as syntax and closes the block early.
#[derive(Debug, Default)]
struct BalanceScan {
    /// Open `"` or `` ` `` literal. Backticks matter because a JavaScript
    /// template literal spans lines and may contain a brace — an unhandled
    /// ``const marker = `}`;`` closes the block early and truncates the range.
    in_string: Option<char>,
    in_block_comment: bool,
}

impl BalanceScan {
    /// Consume `text` and return its net `{` minus `}`, carrying string and
    /// block-comment state into the next call.
    ///
    /// Deliberately not a parser. It has to be right about ordinary code and
    /// conservative everywhere else, because being wrong here means a mangled
    /// file rather than a rejected edit — so anything it cannot account for
    /// leaves the balance non-zero and the caller refuses.
    fn consume(&mut self, text: &str) -> i32 {
        let mut depth = 0_i32;
        let mut chars = text.chars().peekable();
        // Unlike string and comment state, this resets each line: an
        // unterminated `'` is almost always a lifetime (`&'a str`), and
        // carrying it forward would swallow the rest of the file.
        let mut in_char = false;
        while let Some(c) = chars.next() {
            if self.in_block_comment {
                if c == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    self.in_block_comment = false;
                }
                continue;
            }
            match c {
                '\\' if self.in_string.is_some() || in_char => {
                    chars.next();
                }
                '"' | '`' if !in_char => match self.in_string {
                    Some(open) if open == c => self.in_string = None,
                    Some(_) => {}
                    None => self.in_string = Some(c),
                },
                '\'' if self.in_string.is_none() => in_char = !in_char,
                '/' if self.in_string.is_none() && !in_char && chars.peek() == Some(&'/') => {
                    for c in chars.by_ref() {
                        if c == '\n' {
                            break;
                        }
                    }
                }
                // Block comments count too. A `/* } */` inside a function body
                // would otherwise close the block early, and the forward scan
                // would return a truncated range — splicing the new block in
                // and leaving the tail of the old one behind.
                '/' if self.in_string.is_none() && !in_char && chars.peek() == Some(&'*') => {
                    chars.next();
                    self.in_block_comment = true;
                }
                '\n' => in_char = false,
                '{' if self.in_string.is_none() && !in_char => depth += 1,
                '}' if self.in_string.is_none() && !in_char => depth -= 1,
                _ => {}
            }
        }
        depth
    }
}

#[cfg(test)]
mod anchor_tests {
    use super::*;

    const FILE: &str = concat!(
        "use std::fmt;\n",
        "\n",
        "pub fn count_words(s: &str) -> usize {\n",
        "    s.split(' ').count()\n",
        "}\n",
        "\n",
        "#[cfg(test)]\n",
        "mod tests {\n",
        "    #[test]\n",
        "    fn empty_input_has_no_words() {\n",
        "        assert_eq!(super::count_words(\"\"), 0);\n",
        "    }\n",
        "}\n",
    );

    #[test]
    fn a_balanced_block_anchors_on_its_own_first_line() {
        let new = "pub fn count_words(s: &str) -> usize {\n    s.split_whitespace().count()\n}";
        let (start, end) = anchor_replacement_block(FILE, new).expect("should anchor");
        assert_eq!(
            &FILE[start..end],
            "pub fn count_words(s: &str) -> usize {\n    s.split(' ').count()\n}\n"
        );
    }

    #[test]
    fn anchoring_never_swallows_the_rest_of_the_file() {
        let new = "pub fn count_words(s: &str) -> usize {\n    s.split_whitespace().count()\n}";
        let (_, end) = anchor_replacement_block(FILE, new).unwrap();
        assert!(
            FILE[end..].contains("mod tests"),
            "the tests below the replaced function must survive; treating a lone \
             new_str as whole-file content is how they get deleted"
        );
    }

    #[test]
    fn an_unbalanced_replacement_is_refused() {
        let new = "pub fn count_words(s: &str) -> usize {\n    s.split_whitespace().count()";
        assert!(
            anchor_replacement_block(FILE, new).is_none(),
            "a block that does not close would leave the file unparseable"
        );
    }

    #[test]
    fn an_ambiguous_anchor_is_refused() {
        let file = "fn helper() {\n    a();\n}\n\nfn helper() {\n    b();\n}\n";
        let new = "fn helper() {\n    c();\n}";
        assert!(
            anchor_replacement_block(file, new).is_none(),
            "two candidate anchors means the target is a guess"
        );
    }

    #[test]
    fn an_anchor_absent_from_the_file_is_refused() {
        let new = "pub fn other_function(s: &str) -> usize {\n    0\n}";
        assert!(anchor_replacement_block(FILE, new).is_none());
    }

    #[test]
    fn a_statement_without_a_block_is_refused() {
        let new = "let x = 1;";
        assert!(
            anchor_replacement_block(FILE, new).is_none(),
            "a bare statement gives no way to know how much it stands in for"
        );
    }

    #[test]
    fn braces_inside_strings_do_not_confuse_the_balance() {
        assert_eq!(brace_balance("let s = \"{{{\";"), 0);
        assert_eq!(brace_balance("// } } }"), 0);
        assert_eq!(brace_balance("let c = '{';"), 0);
    }

    #[test]
    fn a_lifetime_is_not_read_as_an_open_char_literal() {
        assert_eq!(
            brace_balance("fn f<'a>(x: &'a str) {\n}\n"),
            0,
            "treating `'a` as a quote would make every generic function unbalanced"
        );
    }

    #[test]
    fn a_nested_block_closes_at_the_outer_brace() {
        let file = "fn outer() {\n    if x {\n        y();\n    }\n}\n\nfn after() {}\n";
        let new = "fn outer() {\n    z();\n}";
        let (start, end) = anchor_replacement_block(file, new).unwrap();
        assert_eq!(
            &file[start..end],
            "fn outer() {\n    if x {\n        y();\n    }\n}\n"
        );
    }
}

// ─── Whitespace-tolerant anchor resolution ──────────────────────────────────

/// Find the one span of `file_content` that equals `old_str` once both sides
/// have their indentation and inter-token spacing normalized.
///
/// The exact matcher is right to be strict — it is what makes an applied edit
/// mean "this text was there". But the anchors a small model writes are
/// reconstructed from what it read, and it reproduces the *code* far more
/// reliably than the *spacing*: tabs become spaces, indentation shifts by two,
/// a trailing space disappears. On qwen2.5-coder:7b, "`old_str` was not found
/// exactly as written" is the single most common edit failure
/// (`plans/evidence/production/BENCH/baseline-raw-v1.md`), and in the failures
/// inspected the anchor differed from the file only in whitespace.
///
/// The result is still an exact edit: this only *locates* the span, and the
/// bytes replaced are the file's own. Matching stays line-aligned and must be
/// unique, so a near-miss cannot silently retarget the edit.
///
/// Returns the byte range in `file_content`, or `None` when there is no match
/// or more than one.
pub fn find_whitespace_insensitive(file_content: &str, old_str: &str) -> Option<(usize, usize)> {
    let mut needle_normalizer = LineNormalizer::default();
    let needle: Vec<String> = old_str
        .lines()
        .map(|line| needle_normalizer.normalize(line))
        .filter(|line| !line.is_empty())
        .collect();
    if needle.is_empty() {
        return None;
    }

    // Line starts, with each line's normalized form, so a match can be mapped
    // back to real byte offsets.
    let mut starts: Vec<usize> = Vec::new();
    let mut ends: Vec<usize> = Vec::new();
    let mut normalized: Vec<String> = Vec::new();
    let mut offset = 0_usize;
    let mut file_normalizer = LineNormalizer::default();
    for line in file_content.split_inclusive('\n') {
        starts.push(offset);
        offset += line.len();
        ends.push(offset);
        normalized.push(file_normalizer.normalize(line));
    }

    let mut found: Option<(usize, usize)> = None;
    // Indexes into three parallel vectors (`starts`, `ends`, `normalized`), so
    // the index itself is the loop's subject rather than an artefact.
    #[allow(clippy::needless_range_loop)]
    for begin in 0..normalized.len() {
        // Walk the file from `begin`, skipping blank lines, and see whether the
        // needle's non-blank lines appear in order.
        let mut cursor = begin;
        let mut matched = 0_usize;
        let mut last = begin;
        while matched < needle.len() && cursor < normalized.len() {
            if normalized[cursor].is_empty() {
                // A blank line inside the matched region is tolerated only
                // once the match has started, so a run of blanks before the
                // anchor cannot be absorbed into it.
                if matched == 0 {
                    break;
                }
                cursor += 1;
                continue;
            }
            if normalized[cursor] != needle[matched] {
                break;
            }
            matched += 1;
            last = cursor;
            cursor += 1;
        }
        if matched == needle.len() {
            if found.is_some() {
                return None;
            }
            found = Some((starts[begin], ends[last]));
        }
    }
    found
}

/// Line normalizer whose literal state survives between lines.
///
/// A Python triple-quoted string or a JavaScript template literal is still a
/// literal on its second line. Restarting per line would collapse
/// its content as if it were indentation, so `"  b"` and `" b"` — different
/// string values — would normalize to the same thing and match each other.
#[derive(Debug, Default)]
struct LineNormalizer {
    in_literal: Option<char>,
}

impl LineNormalizer {
    /// Collapse `line` to its significant content: no leading or trailing
    /// whitespace, and every internal run of spaces or tabs reduced to one
    /// space — except inside a literal, where spacing is content and is kept
    /// exactly.
    fn normalize(&mut self, line: &str) -> String {
        // Inside a multi-line literal the whole line is content, indentation
        // included, so it is taken verbatim rather than trimmed.
        let body = if self.in_literal.is_some() {
            line.trim_end_matches(['\n', '\r'])
        } else {
            line.trim()
        };
        let mut out = String::with_capacity(body.len());
        let mut pending_space = false;
        let mut escaped = false;
        let mut chars = body.chars().peekable();
        while let Some(c) = chars.next() {
            if let Some(delimiter) = self.in_literal {
                out.push(c);
                if escaped {
                    escaped = false;
                } else if c == '\\' {
                    escaped = true;
                } else if c == delimiter {
                    self.in_literal = None;
                }
                continue;
            }
            if c == ' ' || c == '\t' || c == '\r' {
                pending_space = true;
                continue;
            }
            if pending_space && !out.is_empty() {
                out.push(' ');
            }
            pending_space = false;

            // A line comment ends the line's lexical structure. Reading on
            // would let an apostrophe in prose — `// don't` — open a literal
            // that swallows the *next* line, whose real literal then closes
            // the false one and has its spacing collapsed as if it were
            // formatting.
            if c == '/' && chars.peek() == Some(&'/') {
                out.push(c);
                out.extend(chars);
                break;
            }
            if c == '#' {
                out.push(c);
                out.extend(chars);
                break;
            }

            // Backticks count: a JavaScript template literal spans lines and
            // its contents are content, not formatting.
            //
            // A `'` that opens no literal — a Rust lifetime — leaves the rest
            // of the text treated as literal content. That only makes matching
            // stricter: missing a match is recoverable, matching the wrong
            // line is not.
            if c == '"' || c == '\'' || c == '`' {
                self.in_literal = Some(c);
            }
            out.push(c);
        }
        out
    }
}

#[cfg(test)]
mod whitespace_tests {
    use super::*;

    const FILE: &str = concat!(
        "fn main() {\n",
        "    let total = compute(\n",
        "        a,\n",
        "        b,\n",
        "    );\n",
        "}\n",
    );

    #[test]
    fn indentation_drift_still_matches() {
        let anchor = "let total = compute(\na,\nb,\n);";
        let (start, end) = find_whitespace_insensitive(FILE, anchor).expect("should locate");
        assert_eq!(
            &FILE[start..end],
            "    let total = compute(\n        a,\n        b,\n    );\n",
            "the span returned is the file's own bytes, so the edit stays exact"
        );
    }

    #[test]
    fn tabs_and_spaces_are_the_same_anchor() {
        let file = "fn f() {\n\tlet x = 1;\n}\n";
        let (start, end) = find_whitespace_insensitive(file, "    let x = 1;").unwrap();
        assert_eq!(&file[start..end], "\tlet x = 1;\n");
    }

    #[test]
    fn a_real_difference_is_not_matched() {
        assert!(
            find_whitespace_insensitive(FILE, "let total = compute(x)").is_none(),
            "tolerating whitespace must not tolerate different code"
        );
    }

    #[test]
    fn an_ambiguous_anchor_is_refused() {
        let file = "fn a() {\n    x();\n}\n\nfn b() {\n    x();\n}\n";
        assert!(
            find_whitespace_insensitive(file, "x();").is_none(),
            "two matches means the edit would be applied to a guess"
        );
    }

    #[test]
    fn an_empty_anchor_never_matches() {
        assert!(find_whitespace_insensitive(FILE, "   \n  \n").is_none());
    }

    #[test]
    fn internal_spacing_is_normalized() {
        let file = "fn f() {\n    let x  =   1;\n}\n";
        assert!(find_whitespace_insensitive(file, "let x = 1;").is_some());
    }

    #[test]
    fn a_blank_line_before_the_anchor_is_not_absorbed() {
        let file = "fn f() {\n\n    let x = 1;\n}\n";
        let (start, _) = find_whitespace_insensitive(file, "let x = 1;").unwrap();
        assert_eq!(
            &file[start..start + 4],
            "    ",
            "the match must begin at the anchor's line, not at the blank above it"
        );
    }

    #[test]
    fn carriage_returns_do_not_defeat_the_match() {
        let file = "fn f() {\r\n    let x = 1;\r\n}\r\n";
        assert!(
            find_whitespace_insensitive(file, "let x = 1;").is_some(),
            "a CRLF file must match an anchor written with LF"
        );
    }
}

#[cfg(test)]
mod review_regression_tests {
    use super::*;

    /// A `}` inside a block comment must not close the block.
    ///
    /// Counting it would truncate the replacement range, so the new block gets
    /// spliced in and the tail of the old one is left behind — a proposal that
    /// does not compile, produced silently.
    #[test]
    fn a_brace_in_a_block_comment_does_not_close_the_block() {
        let file = concat!(
            "fn target() {\n",
            "    /* the closing } here is prose */\n",
            "    old();\n",
            "}\n",
            "\n",
            "fn after() {}\n",
        );
        let new = "fn target() {\n    fresh();\n}";
        let (start, end) = anchor_replacement_block(file, new).expect("should anchor");
        assert_eq!(
            &file[start..end],
            "fn target() {\n    /* the closing } here is prose */\n    old();\n}\n",
            "the range must cover the whole function, not stop at the comment"
        );
        let spliced = splice_replacement(file, start, end, new);
        assert!(
            !spliced.contains("old();"),
            "a truncated range leaves the old body behind: {spliced:?}"
        );
        assert!(spliced.contains("fn after() {}"));
    }

    #[test]
    fn a_multi_line_block_comment_is_skipped_whole() {
        assert_eq!(brace_balance("/*\n}\n}\n*/\n"), 0);
        assert_eq!(
            brace_balance("fn f() { /* } */ }"),
            0,
            "one real open and one real close, with a decoy between them"
        );
    }

    /// Whitespace inside a string literal is content, not formatting.
    ///
    /// `"a b"` and `"a  b"` are different strings. Collapsing both to the same
    /// normalized form would let an anchor match a line whose code it does not
    /// equal, and the edit would be applied anyway.
    #[test]
    fn spacing_inside_a_string_literal_is_not_collapsed() {
        let file = "fn f() {\n    let s = \"a  b\";\n}\n";
        assert!(
            find_whitespace_insensitive(file, "let s = \"a b\";").is_none(),
            "two different string literals must not be treated as one anchor"
        );
    }

    #[test]
    fn spacing_outside_a_literal_is_still_collapsed() {
        let file = "fn f() {\n    let s   =  \"a  b\";\n}\n";
        assert!(
            find_whitespace_insensitive(file, "let s = \"a  b\";").is_some(),
            "the literal matches exactly; only the code around it drifted"
        );
    }

    #[test]
    fn an_escaped_quote_does_not_end_the_literal() {
        let file = "fn f() {\n    let s = \"a\\\"  b\";\n}\n";
        assert!(
            find_whitespace_insensitive(file, "let s = \"a\\\" b\";").is_none(),
            "the escaped quote keeps the literal open, so its spacing stays significant"
        );
    }
}

#[cfg(test)]
mod cross_line_state_tests {
    use super::*;

    /// The forward scan must remember an open `/*` from a previous line.
    ///
    /// The earlier regression test for this called `brace_balance` on the whole
    /// comment at once, so it never exercised the line-by-line scan that
    /// `anchor_replacement_block` actually uses — it passed while the bug was
    /// still there.
    #[test]
    fn a_block_comment_spanning_lines_does_not_close_the_block() {
        let file = concat!(
            "fn target() {\n",
            "    /* a comment\n",
            "       whose second line has a } in it\n",
            "       and a third line */\n",
            "    old();\n",
            "}\n",
            "\n",
            "fn after() {}\n",
        );
        let new = "fn target() {\n    fresh();\n}";
        let (start, end) = anchor_replacement_block(file, new).expect("should anchor");
        assert!(
            file[start..end].contains("old();"),
            "the range must reach past the comment to the real closing brace, \
             got {:?}",
            &file[start..end]
        );
        let spliced = splice_replacement(file, start, end, new);
        assert!(!spliced.contains("old();"));
        assert!(!spliced.contains("whose second line"));
        assert!(spliced.contains("fn after() {}"));
    }

    #[test]
    fn balance_state_carries_between_consumed_chunks() {
        let mut scan = BalanceScan::default();
        assert_eq!(scan.consume("fn f() { /* open\n"), 1);
        assert_eq!(
            scan.consume("} still comment */\n"),
            0,
            "the brace is inside a comment that started on the previous chunk"
        );
        assert_eq!(scan.consume("}\n"), -1);
    }

    /// Fuzzy matching must not collapse the interior of a multi-line literal.
    ///
    /// Python's triple-quoted strings span lines, so per-line normalization
    /// treated their content as indentation: `"  b"` and `" b"` — different
    /// string values — normalized to the same thing and matched each other.
    #[test]
    fn spacing_inside_a_multi_line_literal_is_not_collapsed() {
        let file = concat!(
            "def f():\n",
            "    s = \"\"\"\n",
            "  b\n",
            "\"\"\"\n",
            "    return s\n",
        );
        // Same shape, but one space where the file has two.
        let anchor = "s = \"\"\"\n b\n\"\"\"";
        assert!(
            find_whitespace_insensitive(file, anchor).is_none(),
            "two different string values must not be treated as one anchor"
        );
    }

    #[test]
    fn a_multi_line_literal_still_matches_itself() {
        let file = concat!(
            "def f():\n",
            "    s = \"\"\"\n",
            "  b\n",
            "\"\"\"\n",
            "    return s\n",
        );
        let anchor = "s   =  \"\"\"\n  b\n\"\"\"";
        assert!(
            find_whitespace_insensitive(file, anchor).is_some(),
            "the literal's content is identical; only the code around it drifted"
        );
    }
}

#[cfg(test)]
mod lexical_tests {
    use super::*;

    /// The anchor guard must ask the same question the forward scan answers.
    ///
    /// A textual `contains('{')` accepts a line whose brace lives in a string.
    /// The scanner correctly ignores that brace, so the scan sees depth zero
    /// immediately, ends the range on the first line, and the splice keeps
    /// whatever followed — a silently truncated replacement.
    #[test]
    fn a_brace_inside_a_string_does_not_open_a_block() {
        let file = concat!("const marker = \"{\";\n", "old_one();\n", "old_two();\n",);
        let new = "const marker = \"{\";\nfresh();";
        assert!(
            anchor_replacement_block(file, new).is_none(),
            "this line opens no block, so there is no block to replace"
        );
    }

    #[test]
    fn a_real_opening_brace_still_anchors() {
        let file = "fn target() {\n    old();\n}\n\nfn after() {}\n";
        let new = "fn target() {\n    fresh();\n}";
        assert!(anchor_replacement_block(file, new).is_some());
    }

    /// A JavaScript template literal spans lines and may contain a brace.
    #[test]
    fn a_brace_in_a_template_literal_does_not_close_the_block() {
        let file = concat!(
            "function target() {\n",
            "  const marker = `}`;\n",
            "  old();\n",
            "}\n",
            "\n",
            "function after() {}\n",
        );
        let new = "function target() {\n  fresh();\n}";
        let (start, end) = anchor_replacement_block(file, new).expect("should anchor");
        assert!(
            file[start..end].contains("old();"),
            "the range must reach the real closing brace, got {:?}",
            &file[start..end]
        );
        assert!(splice_replacement(file, start, end, new).contains("function after() {}"));
    }

    #[test]
    fn template_state_carries_between_chunks() {
        let mut scan = BalanceScan::default();
        assert_eq!(scan.consume("const t = `open\n"), 0);
        assert_eq!(
            scan.consume("} still template`;\n"),
            0,
            "the brace is inside a template opened on the previous chunk"
        );
    }

    /// Spacing inside a template literal is content, not formatting.
    #[test]
    fn spacing_inside_a_template_literal_is_not_collapsed() {
        let file = "function f() {\n  const s = `a  b`;\n}\n";
        assert!(
            find_whitespace_insensitive(file, "const s = `a b`;").is_none(),
            "two different template values must not be treated as one anchor"
        );
    }

    /// An apostrophe in a comment must not open a literal.
    ///
    /// Carrying that false state into the next line lets a real literal there
    /// close it, and the real literal's spacing is then collapsed as if it
    /// were indentation — so a semantically different string matches.
    #[test]
    fn an_apostrophe_in_a_comment_does_not_open_a_literal() {
        let file = "// don't\nlet s = 'a  b';\n";
        assert!(
            find_whitespace_insensitive(file, "// don't\nlet s = 'a b';").is_none(),
            "the two string values differ, so this must not match"
        );
    }

    #[test]
    fn a_comment_does_not_break_an_otherwise_valid_match() {
        let file = "// don't\nlet s = 'a  b';\n";
        assert!(
            find_whitespace_insensitive(file, "// don't\nlet s = 'a  b';").is_some(),
            "identical content must still match through a comment"
        );
    }
}
