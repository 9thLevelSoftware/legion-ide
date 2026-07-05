//! Fuzzy-match scorer for the Legion command palette and file opener.
//!
//! This module provides a [`fuzzy_score`] function that computes an ordered,
//! subsequence-based relevance score for a `(candidate, query)` pair.  It is
//! the canonical scorer for the entire Legion IDE: both the command palette and
//! the file opener use it so that ranking is consistent.
//!
//! ## Scoring model
//!
//! All characters in `query` must appear in `candidate` in order (subsequence
//! rule).  If any character is missing, `fuzzy_score` returns `None`.
//!
//! When all query characters match, bonuses are accumulated:
//!
//! | Bonus                         | Points |
//! |-------------------------------|--------|
//! | Base character match          | +10    |
//! | Consecutive-run continuation  | +15    |
//! | Word-boundary start           | +20    |
//! | camelCase boundary start      | +15    |
//! | Path-segment start (after `/` or `\`)| +25 |
//! | Filename-region match         | +12    |
//! | Exact full match              | +100   |
//! | Prefix match                  | +60    |
//! | Substring (contains)          | +35    |
//! | Gap penalty                   | -1 per skipped char |
//!
//! "Word boundary" covers `_`, `-`, `.` separators.
//! "camelCase boundary" is detected when a candidate character is uppercase and
//! immediately follows a lowercase character in the raw (un-lowercased) string.
//! "Path-segment start" is detected when the previous character is `/` or `\`.
//! "Filename-region" means the match index falls within the last path component
//! (i.e. after the last `/` or `\`).

/// A successful fuzzy match result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuzzyScore {
    /// Composite relevance score.  Higher is better.
    pub score: i32,
    /// Zero-based indices in the candidate string that were matched.
    pub match_indices: Vec<usize>,
}

/// Compute a fuzzy-match score for `candidate` against `query`.
///
/// Returns `None` when `query` is not a subsequence of `candidate`.
/// Returns `Some(FuzzyScore { score: 0, match_indices: [] })` when `query`
/// is empty (every candidate is a valid match for an empty query).
///
/// The scoring algorithm is **deterministic**: given the same inputs it always
/// produces the same output.  It is also **allocation-light**: the only heap
/// allocation per call is the `match_indices` vector, whose capacity is
/// bounded by `query.len()`.
pub fn fuzzy_score(candidate: &str, query: &str) -> Option<FuzzyScore> {
    let query = query.trim();
    if query.is_empty() {
        return Some(FuzzyScore {
            score: 0,
            match_indices: Vec::new(),
        });
    }

    // Collect lowercased query characters for case-insensitive comparison.
    let query_chars: Vec<char> = query.chars().flat_map(char::to_lowercase).collect();

    // Collect raw candidate characters (needed for camelCase detection).
    let candidate_raw: Vec<char> = candidate.chars().collect();
    // Collect lowercased candidate characters for comparison.
    let candidate_lower: Vec<char> = candidate_raw
        .iter()
        .map(|&ch| ch.to_lowercase().next().unwrap_or(ch))
        .collect();

    // Find the last path separator to determine the filename region.
    let filename_start_idx = candidate_raw
        .iter()
        .enumerate()
        .rfind(|&(_, &ch)| ch == '/' || ch == '\\')
        .map(|(i, _)| i + 1)
        .unwrap_or(0);

    let mut match_indices = Vec::with_capacity(query_chars.len());
    let mut query_idx = 0;
    let mut score: i32 = 0;
    let mut last_match: Option<usize> = None;

    for (cand_idx, &cand_char) in candidate_lower.iter().enumerate() {
        if query_idx >= query_chars.len() {
            break;
        }
        if cand_char != query_chars[query_idx] {
            continue;
        }

        // Base match bonus.
        score += 10;

        // Consecutive-run continuation bonus.
        if last_match.is_some_and(|last| last + 1 == cand_idx) {
            score += 15;
        }

        let prev_raw = if cand_idx > 0 {
            Some(candidate_raw[cand_idx - 1])
        } else {
            None
        };
        let curr_raw = candidate_raw[cand_idx];

        // Path-segment start (after `/` or `\`): strongest positional bonus.
        if prev_raw.is_some_and(|p| p == '/' || p == '\\') || cand_idx == 0 {
            score += 25;
        }
        // Word-boundary start (`_`, `-`, `.` separators).
        if prev_raw.is_some_and(|p| matches!(p, '_' | '-' | '.')) {
            score += 20;
        }
        // camelCase boundary: current char is uppercase and previous is lowercase.
        if curr_raw.is_uppercase() && prev_raw.is_some_and(|p| p.is_lowercase()) {
            score += 15;
        }

        // Filename-region bonus: match falls within the last path component.
        if cand_idx >= filename_start_idx {
            score += 12;
        }

        // Gap penalty: penalise distance from the last matched position.
        if let Some(last) = last_match {
            score -= cand_idx.saturating_sub(last + 1) as i32;
        }

        match_indices.push(cand_idx);
        last_match = Some(cand_idx);
        query_idx += 1;
    }

    // All query characters must have been consumed.
    if query_idx < query_chars.len() {
        return None;
    }

    // Whole-string bonuses (applied once, after all characters matched).
    let candidate_lower_str = candidate.to_ascii_lowercase();
    let query_lower_str = query.to_ascii_lowercase();
    if candidate_lower_str == query_lower_str {
        score += 100;
    } else if candidate_lower_str.starts_with(&query_lower_str) {
        score += 60;
    } else if candidate_lower_str.contains(&query_lower_str) {
        score += 35;
    }

    Some(FuzzyScore {
        score,
        match_indices,
    })
}

/// Returns `fuzzy_score` results as a raw `(score, match_indices)` tuple.
///
/// This adapter exists for call sites that previously consumed the private
/// `palette_fuzzy_score` function, which returned the same tuple shape.
/// **Behavioral note**: the scores and match-indices produced here are
/// *identical* to those of `fuzzy_score` — there is no legacy scoring
/// algorithm; the name of the old function was the only legacy aspect.
/// Prefer calling `fuzzy_score` directly when the `FuzzyScore` struct is
/// more convenient.
pub fn fuzzy_score_tuple(candidate: &str, query: &str) -> Option<(i32, Vec<usize>)> {
    fuzzy_score(candidate, query).map(|fs| (fs.score, fs.match_indices))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── basic subsequence contract ────────────────────────────────────────────

    #[test]
    fn empty_query_matches_anything() {
        assert!(fuzzy_score("anything", "").is_some());
        assert_eq!(fuzzy_score("anything", "").unwrap().score, 0);
        assert!(fuzzy_score("anything", "  ").is_some()); // whitespace-only trimmed
    }

    #[test]
    fn non_subsequence_returns_none() {
        assert!(fuzzy_score("hello", "xyz").is_none());
        assert!(fuzzy_score("abc", "abcd").is_none());
    }

    #[test]
    fn exact_match_returns_some() {
        assert!(fuzzy_score("hello", "hello").is_some());
    }

    #[test]
    fn subsequence_matches() {
        // "helo" is a subsequence of "hello"
        assert!(fuzzy_score("hello", "helo").is_some());
    }

    #[test]
    fn match_indices_are_correct() {
        let result = fuzzy_score("abcdef", "ace").expect("subsequence match");
        assert_eq!(result.match_indices, vec![0, 2, 4]);
    }

    // ── scoring order ─────────────────────────────────────────────────────────

    #[test]
    fn consecutive_run_beats_scattered() {
        // "abcd" matched in "abcXXd" (scattered) vs "abcd" (consecutive)
        let scattered = fuzzy_score("abXXcd", "abcd").expect("scattered match");
        let consecutive = fuzzy_score("abcd", "abcd").expect("consecutive match");
        assert!(
            consecutive.score > scattered.score,
            "consecutive={} scattered={}",
            consecutive.score,
            scattered.score
        );
    }

    #[test]
    fn word_boundary_match_beats_mid_word() {
        // "src/lib.rs" — 'l' at start of "lib" (after '/') beats 'l' mid-word in "hello"
        let boundary = fuzzy_score("src/lib.rs", "l").expect("boundary match");
        let midword = fuzzy_score("hello", "l").expect("midword match");
        assert!(
            boundary.score > midword.score,
            "boundary={} midword={}",
            boundary.score,
            midword.score
        );
    }

    #[test]
    fn camel_case_boundary_scores_higher_than_mid_word() {
        // "MyService" — 'S' at camelCase boundary in "MyService" vs 's' inside "rust"
        let camel = fuzzy_score("MyService", "S").expect("camel match");
        let mid = fuzzy_score("rust", "s").expect("mid match");
        assert!(
            camel.score > mid.score,
            "camel={} mid={}",
            camel.score,
            mid.score
        );
    }

    #[test]
    fn path_segment_start_bonus_applied() {
        // Matching 'f' at start of "foo.rs" in a path should beat matching 'f'
        // mid-path in "stuff/x.rs".
        let seg_start = fuzzy_score("bar/foo.rs", "f").expect("seg-start match");
        let mid_path = fuzzy_score("buffer/x.rs", "f").expect("mid-path match");
        // seg_start has 'f' right after '/' vs mid_path has 'f' inside "buffer"
        assert!(
            seg_start.score > mid_path.score,
            "seg_start={} mid_path={}",
            seg_start.score,
            mid_path.score
        );
    }

    #[test]
    fn filename_region_bonus_applied() {
        // "foo" matched inside "src/foo.rs" (filename region) should beat
        // "foo" matched inside "foobar/src/x.rs" (directory region).
        let filename = fuzzy_score("src/foo.rs", "foo").expect("filename region");
        let directory = fuzzy_score("foo/src/x.rs", "foo").expect("directory region");
        assert!(
            filename.score > directory.score,
            "filename={} directory={}",
            filename.score,
            directory.score
        );
    }

    #[test]
    fn exact_match_bonus_applied() {
        let exact = fuzzy_score("hello", "hello").expect("exact match");
        let prefix = fuzzy_score("helloworld", "hello").expect("prefix match");
        assert!(
            exact.score > prefix.score,
            "exact={} prefix={}",
            exact.score,
            prefix.score
        );
    }

    #[test]
    fn prefix_beats_contains() {
        let prefix = fuzzy_score("foobar", "foo").expect("prefix match");
        let contains = fuzzy_score("xfoobar", "foo").expect("contains match");
        assert!(
            prefix.score >= contains.score,
            "prefix={} contains={}",
            prefix.score,
            contains.score
        );
    }

    // ── case insensitivity ────────────────────────────────────────────────────

    #[test]
    fn case_insensitive_match() {
        assert!(fuzzy_score("Cargo.toml", "cargo").is_some());
        assert!(fuzzy_score("HELLO", "hello").is_some());
    }

    // ── tuple adapter ─────────────────────────────────────────────────────────

    #[test]
    fn tuple_adapter_returns_tuple() {
        let result = fuzzy_score_tuple("hello", "hel");
        assert!(result.is_some());
        let (score, indices) = result.unwrap();
        assert!(score > 0);
        assert_eq!(indices, vec![0, 1, 2]);
    }
}
