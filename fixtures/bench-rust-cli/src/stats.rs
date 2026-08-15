//! Text statistics for wordtally.

use std::collections::HashMap;

/// Aggregate statistics for one input text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Summary {
    pub lines: usize,
    pub words: usize,
    pub chars: usize,
    /// Case-insensitive word frequencies, most frequent first.
    pub word_freq: Vec<(String, usize)>,
}

/// Compute every statistic wordtally reports for `text`.
pub fn summarize(text: &str) -> Summary {
    Summary {
        lines: count_lines(text),
        words: count_words(text),
        chars: text.chars().count(),
        word_freq: word_frequencies(text),
    }
}

/// Number of lines in `text`, as delimited by `\n`.
pub fn count_lines(text: &str) -> usize {
    text.lines().count()
}

/// Number of whitespace-separated words in `text`.
pub fn count_words(text: &str) -> usize {
    text.split(' ').count()
}

/// Case-insensitive word frequencies, most frequent first; ties broken
/// alphabetically so the ordering is deterministic. Punctuation is stripped
/// and words are lowercased before counting.
pub fn word_frequencies(text: &str) -> Vec<(String, usize)> {
    let mut freq: HashMap<String, usize> = HashMap::new();
    for word in text.split_whitespace() {
        let normalized: String = word
            .chars()
            .filter(|c| c.is_alphanumeric())
            .flat_map(|c| c.to_lowercase())
            .collect();
        if normalized.is_empty() {
            continue;
        }
        *freq.entry(normalized).or_insert(0) += 1;
    }
    let mut pairs: Vec<(String, usize)> = freq.into_iter().collect();
    pairs.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
    pairs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counts_lines() {
        assert_eq!(count_lines("one\ntwo\nthree\n"), 3);
        assert_eq!(count_lines(""), 0);
    }

    #[test]
    fn counts_simple_words() {
        assert_eq!(count_words("alpha beta gamma"), 3);
    }

    #[test]
    fn empty_input_has_no_words() {
        assert_eq!(count_words(""), 0);
    }

    #[test]
    fn repeated_spaces_do_not_add_words() {
        assert_eq!(count_words("alpha  beta"), 2);
    }

    #[test]
    fn newlines_separate_words() {
        assert_eq!(count_words("alpha\nbeta gamma"), 3);
    }

    #[test]
    fn frequencies_sorted_by_count_then_name() {
        let freq = word_frequencies("b a b c a b");
        assert_eq!(
            freq,
            vec![
                ("b".to_string(), 3),
                ("a".to_string(), 2),
                ("c".to_string(), 1),
            ]
        );
    }

    #[test]
    fn frequencies_normalize_case_and_punctuation() {
        let freq = word_frequencies("Stop! stop STOP.");
        assert_eq!(freq, vec![("stop".to_string(), 3)]);
    }
}
