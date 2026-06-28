//! Helpers for splitting streamed markdown into text and code-block segments.

/// Markdown segment emitted by a streaming assistant response.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MarkdownStreamSegment {
    /// Plain markdown text outside of fenced code blocks.
    Text(String),
    /// Fenced code block content and language hint.
    CodeBlock {
        /// Optional language label from the opening fence.
        language: Option<String>,
        /// Raw code block body without the fence markers.
        code: String,
        /// Whether a closing fence was observed.
        complete: bool,
    },
}

/// Splits a markdown stream into alternating text and fenced code-block segments.
#[must_use]
pub fn split_markdown_stream(input: &str) -> Vec<MarkdownStreamSegment> {
    let mut segments = Vec::new();
    let mut text_lines: Vec<&str> = Vec::new();
    let mut code_lines: Vec<&str> = Vec::new();
    let mut code_language: Option<String> = None;
    let mut in_code_block = false;

    let flush_text = |segments: &mut Vec<MarkdownStreamSegment>, text_lines: &mut Vec<&str>| {
        if text_lines.is_empty() {
            return;
        }
        segments.push(MarkdownStreamSegment::Text(text_lines.join("")));
        text_lines.clear();
    };

    let flush_code = |
        segments: &mut Vec<MarkdownStreamSegment>,
        code_lines: &mut Vec<&str>,
        code_language: &mut Option<String>,
        complete: bool,
    | {
        // Empty code blocks (e.g. a placeholder fence with only a language
        // label) are intentionally preserved: an opening fence was observed,
        // so the segment is real even when it carries no body.
        let mut code = code_lines.join("");
        if complete && code.ends_with('\n') {
            code.pop();
            if code.ends_with('\r') {
                code.pop();
            }
        }
        segments.push(MarkdownStreamSegment::CodeBlock {
            language: code_language.take(),
            code,
            complete,
        });
        code_lines.clear();
    };

    for line in input.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if trimmed.starts_with("```") {
            if in_code_block {
                flush_code(&mut segments, &mut code_lines, &mut code_language, true);
                in_code_block = false;
            } else {
                flush_text(&mut segments, &mut text_lines);
                let language = trimmed
                    .trim_start_matches("```")
                    .split_whitespace()
                    .next()
                    .filter(|label| !label.is_empty())
                    .map(|label| label.to_string());
                code_language = language;
                in_code_block = true;
            }
            continue;
        }

        if in_code_block {
            code_lines.push(line);
        } else {
            text_lines.push(line);
        }
    }

    if in_code_block {
        flush_code(&mut segments, &mut code_lines, &mut code_language, false);
    } else {
        flush_text(&mut segments, &mut text_lines);
    }

    segments
}

#[cfg(test)]
mod tests {
    use super::{MarkdownStreamSegment, split_markdown_stream};

    #[test]
    fn split_markdown_stream_preserves_complete_code_block_segments() {
        let segments = split_markdown_stream("before\n\n```rust\nfn demo() {}\n```\nafter\n");

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], MarkdownStreamSegment::Text("before\n\n".to_string()));
        assert_eq!(
            segments[1],
            MarkdownStreamSegment::CodeBlock {
                language: Some("rust".to_string()),
                code: "fn demo() {}".to_string(),
                complete: true,
            }
        );
        assert_eq!(segments[2], MarkdownStreamSegment::Text("after\n".to_string()));
    }

    #[test]
    fn split_markdown_stream_preserves_empty_code_block_segments() {
        let segments = split_markdown_stream("before\n```rust\n```\nafter\n");

        assert_eq!(segments.len(), 3);
        assert_eq!(segments[0], MarkdownStreamSegment::Text("before\n".to_string()));
        assert_eq!(
            segments[1],
            MarkdownStreamSegment::CodeBlock {
                language: Some("rust".to_string()),
                code: String::new(),
                complete: true,
            }
        );
        assert_eq!(segments[2], MarkdownStreamSegment::Text("after\n".to_string()));
    }

    #[test]
    fn split_markdown_stream_preserves_empty_open_code_block_segments() {
        let segments = split_markdown_stream("intro\n```python\n");

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], MarkdownStreamSegment::Text("intro\n".to_string()));
        assert_eq!(
            segments[1],
            MarkdownStreamSegment::CodeBlock {
                language: Some("python".to_string()),
                code: String::new(),
                complete: false,
            }
        );
    }

    #[test]
    fn split_markdown_stream_marks_open_code_block_segments_incomplete() {
        let segments = split_markdown_stream("intro\n```python\nprint('hi')\n");

        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0], MarkdownStreamSegment::Text("intro\n".to_string()));
        assert_eq!(
            segments[1],
            MarkdownStreamSegment::CodeBlock {
                language: Some("python".to_string()),
                code: "print('hi')\n".to_string(),
                complete: false,
            }
        );
    }
}
