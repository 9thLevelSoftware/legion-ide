use legion_ai::streaming::{MarkdownStreamSegment, split_markdown_stream};

#[test]
fn split_markdown_stream_extracts_text_and_code_blocks() {
    let segments = split_markdown_stream("before\n\n```rust\nfn demo() {}\n```\nafter\n");

    assert_eq!(segments.len(), 3);
    assert_eq!(
        segments[0],
        MarkdownStreamSegment::Text("before\n\n".to_string())
    );
    assert_eq!(
        segments[1],
        MarkdownStreamSegment::CodeBlock {
            language: Some("rust".to_string()),
            code: "fn demo() {}".to_string(),
            complete: true,
        }
    );
    assert_eq!(
        segments[2],
        MarkdownStreamSegment::Text("after\n".to_string())
    );
}

#[test]
fn split_markdown_stream_marks_open_code_blocks_incomplete() {
    let segments = split_markdown_stream("intro\n```python\nprint('hi')\n");

    assert_eq!(segments.len(), 2);
    assert_eq!(
        segments[0],
        MarkdownStreamSegment::Text("intro\n".to_string())
    );
    assert_eq!(
        segments[1],
        MarkdownStreamSegment::CodeBlock {
            language: Some("python".to_string()),
            code: "print('hi')\n".to_string(),
            complete: false,
        }
    );
}
