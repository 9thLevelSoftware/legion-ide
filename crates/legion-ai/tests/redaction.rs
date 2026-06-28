use legion_ai::redaction::redact_model_bound_output;

#[test]
fn redact_model_bound_output_scrubs_secret_markers_and_truncates() {
    let output = "prefix OPENAI_API_KEY=sk-test-mock Authorization: Bearer mock-token-value trailing context that keeps this payload beyond the byte ceiling";

    let redacted = redact_model_bound_output(output, 48);

    assert!(redacted.redacted_text.contains("[redacted]"));
    assert!(!redacted.redacted_text.contains("sk-test-mock"));
    assert!(!redacted.redacted_text.contains("mock-token-value"));
    assert!(redacted.redacted_text.len() <= 48);
    assert!(redacted.redacted);
    assert!(redacted.truncated);
}

#[test]
fn redact_model_bound_output_preserves_utf8_boundaries() {
    let output = "tool output 😀😀😀";

    let redacted = redact_model_bound_output(output, "tool output 😀".len());

    assert!(
        redacted
            .redacted_text
            .is_char_boundary(redacted.redacted_text.len())
    );
    assert!(redacted.redacted_text.starts_with("tool output "));
    assert!(redacted.byte_count >= output.len() as u64);
}

#[test]
fn redact_model_bound_output_scrubs_new_context_scanning_markers() {
    let output = "proposal_content: retained_context: terminal_excerpts: ejected_context: OPENAI_API_KEY=sk-test-mock";

    let redacted = redact_model_bound_output(output, 256);

    assert!(redacted.redacted);
    assert!(!redacted.truncated);
    assert!(redacted.redacted_text.contains("[redacted]"));
    assert!(!redacted.redacted_text.contains("proposal_content"));
    assert!(!redacted.redacted_text.contains("terminal_excerpts"));
    assert!(!redacted.redacted_text.contains("retained_context"));
    assert!(!redacted.redacted_text.contains("ejected_context"));
}
