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

/// Builds an AWS access key id shaped credential without committing one.
///
/// Generated from a fixed seed so no credential-shaped literal is stored in the
/// repository and no scanner allowlist entry is needed to keep this test.
fn synthetic_access_key_id() -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut state: u64 = 0x2468_ace0_1357_9bdf;
    let body: String = (0..16)
        .map(|_| {
            state = state
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            ALPHABET[((state >> 33) as usize) % ALPHABET.len()] as char
        })
        .collect();
    format!("AKIA{body}")
}

#[test]
fn redact_model_bound_output_scrubs_credentials_with_no_marker_or_prefix() {
    // The marker pass knows `sk-`, `xoxb-`, `ghp_`, `gho_`, and three assignment
    // keywords. An AWS access key id matches none of them, so before the shared
    // ruleset was wired in this payload reached the provider verbatim.
    let key_id = synthetic_access_key_id();
    let output = format!("sts call failed for {key_id} during fetch");

    let redacted = redact_model_bound_output(&output, 256);

    assert!(redacted.redacted);
    assert!(!redacted.redacted_text.contains(key_id.as_str()));
    assert!(redacted.redacted_text.contains("[redacted]"));
    assert!(redacted.redacted_text.starts_with("sts call failed for "));
}

#[test]
fn redact_model_bound_output_leaves_digests_and_identifiers_intact() {
    // Egress posture still must not shred ordinary tool output. Every string
    // here is the shape of something this workspace produces constantly.
    let output = "content_hash=9f2c4a7b1e6d3058af1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f6071 \
                  causality_id=018f0000-0000-7000-8000-300000000005 \
                  manifest:cbf29ce484222325";

    let redacted = redact_model_bound_output(output, 512);

    assert_eq!(redacted.redacted_text, output);
    assert!(!redacted.redacted);
}
