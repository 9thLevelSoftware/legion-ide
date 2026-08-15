//! Errors produced while parsing miniconf text.

use std::fmt;

/// A parse failure, carrying the 1-based line number where it occurred.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseError {
    /// A `[section` header without the closing `]`.
    UnterminatedSection { line: usize },
    /// A `[]` header with nothing (or only whitespace) between the brackets.
    EmptySectionName { line: usize },
    /// An assignment line whose key is empty.
    EmptyKey { line: usize },
    /// A non-comment line with no `=` separator.
    MissingEquals { line: usize },
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::UnterminatedSection { line } => {
                write!(f, "line {line}: section header is missing ']'")
            }
            ParseError::EmptySectionName { line } => {
                write!(f, "line {line}: section name is empty")
            }
            ParseError::EmptyKey { line } => {
                write!(f, "line {line}: key is empty")
            }
            ParseError::MissingEquals { line } => {
                write!(f, "line {line}: expected 'key = value'")
            }
        }
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_includes_line_number() {
        let message = ParseError::MissingEquals { line: 7 }.to_string();
        assert!(message.contains("line 7"), "got: {message}");
    }
}
