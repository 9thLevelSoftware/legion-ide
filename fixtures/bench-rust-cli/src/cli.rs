//! Command-line parsing for wordtally.

pub const USAGE: &str = "usage: wordtally [--lines] [--words] [--chars] [--top N] FILE...";

/// Parsed command-line options.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    pub show_lines: bool,
    pub show_words: bool,
    pub show_chars: bool,
    pub top: usize,
    pub files: Vec<String>,
}

/// Parse raw CLI arguments (without the program name) into `Options`.
///
/// When no display flag is given, all three counters are enabled.
pub fn parse(args: &[String]) -> Result<Options, String> {
    let mut opts = Options::default();
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--lines" => opts.show_words = true,
            "--words" => opts.show_words = true,
            "--chars" => opts.show_chars = true,
            "--top" => {
                let value = iter
                    .next()
                    .ok_or_else(|| "--top requires a value".to_string())?;
                opts.top = value
                    .parse::<usize>()
                    .map_err(|_| format!("invalid --top value: {value}"))?;
            }
            other if other.starts_with("--") => {
                return Err(format!("unknown flag: {other}"));
            }
            path => opts.files.push(path.to_string()),
        }
    }
    if !opts.show_lines && !opts.show_words && !opts.show_chars {
        opts.show_lines = true;
        opts.show_words = true;
        opts.show_chars = true;
    }
    Ok(opts)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_flags_enables_all_counters() {
        let opts = parse(&args(&["notes.txt"])).unwrap();
        assert!(opts.show_lines && opts.show_words && opts.show_chars);
        assert_eq!(opts.files, vec!["notes.txt".to_string()]);
    }

    #[test]
    fn lines_flag_enables_only_lines() {
        let opts = parse(&args(&["--lines", "notes.txt"])).unwrap();
        assert!(opts.show_lines, "--lines must enable the line counter");
        assert!(!opts.show_words, "--lines must not enable the word counter");
        assert!(!opts.show_chars, "--lines must not enable the char counter");
    }

    #[test]
    fn words_flag_enables_only_words() {
        let opts = parse(&args(&["--words", "notes.txt"])).unwrap();
        assert!(!opts.show_lines);
        assert!(opts.show_words);
        assert!(!opts.show_chars);
    }

    #[test]
    fn flags_can_be_combined() {
        let opts = parse(&args(&["--words", "--chars", "notes.txt"])).unwrap();
        assert!(!opts.show_lines);
        assert!(opts.show_words);
        assert!(opts.show_chars);
    }

    #[test]
    fn top_flag_parses_count() {
        let opts = parse(&args(&["--top", "3", "notes.txt"])).unwrap();
        assert_eq!(opts.top, 3);
    }

    #[test]
    fn top_flag_requires_value() {
        assert!(parse(&args(&["--top"])).is_err());
    }

    #[test]
    fn top_flag_rejects_non_numeric_value() {
        assert!(parse(&args(&["--top", "many"])).is_err());
    }

    #[test]
    fn unknown_flag_is_rejected() {
        assert!(parse(&args(&["--frobnicate"])).is_err());
    }
}
