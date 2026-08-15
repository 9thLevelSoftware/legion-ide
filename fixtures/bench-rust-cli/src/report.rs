//! Plain-text rendering of the final report.

use crate::cli::Options;
use crate::stats::Summary;
use std::fmt::Write as _;

/// Render `summary` according to the display flags in `opts`.
pub fn render(summary: &Summary, opts: &Options) -> String {
    let mut out = String::new();
    if opts.show_lines {
        let _ = writeln!(out, "lines: {}", summary.lines);
    }
    if opts.show_words {
        let _ = writeln!(out, "words: {}", summary.words);
    }
    if opts.show_chars {
        let _ = writeln!(out, "chars: {}", summary.chars);
    }
    if opts.top > 0 {
        let _ = writeln!(out, "top {} words:", opts.top);
        for (word, count) in summary.word_freq.iter().take(opts.top) {
            let _ = writeln!(out, "  {count:>5}  {word}");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_counters() -> Options {
        Options {
            show_lines: true,
            show_words: true,
            show_chars: true,
            top: 0,
            files: Vec::new(),
        }
    }

    fn sample_summary() -> Summary {
        Summary {
            lines: 2,
            words: 5,
            chars: 27,
            word_freq: vec![("aa".to_string(), 2), ("bb".to_string(), 1)],
        }
    }

    #[test]
    fn renders_all_counters() {
        let text = render(&sample_summary(), &all_counters());
        assert_eq!(text, "lines: 2\nwords: 5\nchars: 27\n");
    }

    #[test]
    fn renders_only_requested_counters() {
        let opts = Options {
            show_words: true,
            ..Options::default()
        };
        let text = render(&sample_summary(), &opts);
        assert_eq!(text, "words: 5\n");
    }

    #[test]
    fn renders_top_words_truncated() {
        let mut opts = all_counters();
        opts.top = 1;
        let text = render(&sample_summary(), &opts);
        assert!(text.contains("top 1 words:"));
        assert!(text.contains("    2  aa"));
        assert!(!text.contains("bb"));
    }
}
