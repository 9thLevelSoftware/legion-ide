//! Line-oriented parser for the miniconf grammar.
//!
//! Grammar, one entry per line:
//!
//! ```text
//! # comment            ignored, as are blank lines
//! [section]            starts a new section
//! key = value          assigns within the current section
//! ```
//!
//! Keys assigned before any section header land in the root section `""`.

use crate::error::ParseError;
use crate::{Config, Value};

pub(crate) fn parse(text: &str) -> Result<Config, ParseError> {
    let mut config = Config::default();
    let mut section = String::new();
    for (index, raw_line) in text.lines().enumerate() {
        let line_no = index + 1;
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if let Some(rest) = line.strip_prefix('[') {
            let name = rest
                .strip_suffix(']')
                .ok_or(ParseError::UnterminatedSection { line: line_no })?
                .trim();
            if name.is_empty() {
                return Err(ParseError::EmptySectionName { line: line_no });
            }
            section = name.to_string();
            continue;
        }
        let (key, value) = line
            .split_once('=')
            .ok_or(ParseError::MissingEquals { line: line_no })?;
        let key = key.trim();
        if key.is_empty() {
            return Err(ParseError::EmptyKey { line: line_no });
        }
        config.insert(&section, key, Value::new(value.trim()));
    }
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_sections_and_keys() {
        let config = parse("[server]\nhost = localhost\nport = 8080\n").unwrap();
        assert_eq!(config.get("server", "host").unwrap().as_str(), "localhost");
        assert_eq!(config.get("server", "port").unwrap().as_int(), Some(8080));
    }

    #[test]
    fn keys_before_any_section_land_in_root() {
        let config = parse("name = demo\n[extra]\nkind = test\n").unwrap();
        assert_eq!(config.get("", "name").unwrap().as_str(), "demo");
        assert_eq!(config.get("extra", "kind").unwrap().as_str(), "test");
    }

    #[test]
    fn comments_and_blank_lines_are_ignored() {
        let config = parse("# heading\n\nkey = 1\n  # indented comment\n").unwrap();
        assert_eq!(config.get("", "key").unwrap().as_int(), Some(1));
    }

    #[test]
    fn whitespace_around_tokens_is_trimmed() {
        let config = parse("  [ app ]  \n  debug   =   on  \n").unwrap();
        assert_eq!(config.get("app", "debug").unwrap().as_str(), "on");
    }

    #[test]
    fn later_assignment_wins() {
        let config = parse("k = first\nk = second\n").unwrap();
        assert_eq!(config.get("", "k").unwrap().as_str(), "second");
    }

    #[test]
    fn unterminated_section_reports_line() {
        let err = parse("ok = 1\n[broken\n").unwrap_err();
        assert_eq!(err, ParseError::UnterminatedSection { line: 2 });
    }

    #[test]
    fn empty_section_name_is_rejected() {
        let err = parse("[  ]\n").unwrap_err();
        assert_eq!(err, ParseError::EmptySectionName { line: 1 });
    }

    #[test]
    fn missing_equals_is_rejected() {
        let err = parse("[a]\njust a bare line\n").unwrap_err();
        assert_eq!(err, ParseError::MissingEquals { line: 2 });
    }

    #[test]
    fn empty_key_is_rejected() {
        let err = parse(" = value\n").unwrap_err();
        assert_eq!(err, ParseError::EmptyKey { line: 1 });
    }
}
